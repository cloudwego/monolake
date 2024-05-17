use std::{sync::Arc, thread::JoinHandle};

use futures_channel::{
    mpsc::{channel, Receiver, Sender},
    oneshot::{Receiver as OReceiver, Sender as OSender},
};
use futures_util::sink::SinkExt;
use monoio::{blocking::DefaultThreadPool, utils::bind_to_cpu_set};
use service_async::AsyncMakeService;
use tracing::warn;

use super::{Command, Execute, ResultGroup, RuntimeWrapper, Update, WorkerController};
use crate::{config::RuntimeConfig, AnyError};

pub type JoinHandlesWithOutput<FNO> = (Vec<(JoinHandle<()>, OSender<()>)>, Vec<FNO>);

/// Manager is holden by the main thread, and is used to start and control workers.
pub struct Manager<F, LF> {
    runtime_config: RuntimeConfig,
    thread_pool: Option<Box<DefaultThreadPool>>,
    workers: Vec<Sender<Update<F, LF>>>,
}

impl<F, LF> Manager<F, LF>
where
    F: Send + 'static,
    LF: Send + 'static,
{
    #[inline]
    pub fn spawn_workers_async<A>(&mut self) -> Vec<(JoinHandle<()>, OSender<()>)>
    where
        F: AsyncMakeService,
        Command<F, LF>: Execute<A, F::Service>,
    {
        self.spawn_workers_inner(
            |mut finish_rx, rx, _worker_id, _pre_f| {
                move |mut runtime: RuntimeWrapper| {
                    let worker_controller = WorkerController::<F::Service>::default();
                    runtime.block_on(async move {
                        worker_controller.run_controller(rx).await;
                        finish_rx.close();
                    });
                }
            },
            |_| (|| (), ()),
        )
        .0
    }

    #[inline]
    pub fn spawn_workers_async_with_fn<A, FN, FNL, FNO>(
        &mut self,
        f: FN,
    ) -> JoinHandlesWithOutput<FNO>
    where
        F: AsyncMakeService,
        Command<F, LF>: Execute<A, F::Service>,
        FN: Fn(usize) -> (FNL, FNO),
        FNL: Fn() + Send + 'static,
    {
        self.spawn_workers_inner(
            |mut finish_rx, rx, _worker_id, pre_f| {
                move |mut runtime: RuntimeWrapper| {
                    let worker_controller = WorkerController::<F::Service>::default();
                    runtime.block_on(async move {
                        pre_f();
                        worker_controller.run_controller(rx).await;
                        finish_rx.close();
                    });
                }
            },
            f,
        )
    }

    /// Start workers according to runtime config.
    /// Threads JoinHandle will be returned and each factory Sender will
    /// be saved for config updating.
    pub fn spawn_workers_inner<S, SO, FN, FNL, FNO>(
        &mut self,
        fn_lambda: S,
        pre_f: FN,
    ) -> JoinHandlesWithOutput<FNO>
    where
        S: Fn(OReceiver<()>, Receiver<Update<F, LF>>, usize, FNL) -> SO,
        SO: FnOnce(RuntimeWrapper) + Send + 'static,
        FN: Fn(usize) -> (FNL, FNO),
        FNL: Fn() + Send + 'static,
    {
        let cores = if self.runtime_config.cpu_affinity {
            std::thread::available_parallelism().ok()
        } else {
            None
        };

        let runtime_config = Arc::new(self.runtime_config.clone());
        let mut pre_out = Vec::with_capacity(self.runtime_config.worker_threads);
        let out = (0..self.runtime_config.worker_threads)
            .map(|worker_id| {
                let thread_pool = self.thread_pool.clone();
                let (tx, rx) = channel(128);
                let runtime_config = runtime_config.clone();
                let (finish_tx, finish_rx) = futures_channel::oneshot::channel::<()>();
                let (pre_f, fo) = pre_f(worker_id);
                pre_out.push(fo);
                let f = fn_lambda(finish_rx, rx, worker_id, pre_f);
                let handler = std::thread::Builder::new()
                    .name(format!("monolake-worker-{worker_id}"))
                    .spawn(move || {
                        f(RuntimeWrapper::new(
                            runtime_config.as_ref(),
                            thread_pool.map(|p| p as Box<_>),
                        ))
                    })
                    .expect("start worker thread {worker_id} failed");
                // bind thread to cpu core
                if let Some(cores) = cores {
                    let core = worker_id % cores;
                    if let Err(e) = bind_to_cpu_set([core]) {
                        warn!("bind thread {worker_id} to core {core} failed: {e}");
                    }
                }
                self.workers.push(tx);
                (handler, finish_tx)
            })
            .collect();
        (out, pre_out)
    }

    /// Broadcast command to all workers, a Vec of each result will be returned.
    // TODO: Make workers apply command in parallel(use FuturesOrdered).
    // TODO: Return a custom struct(impl Iter) and provide a simple fn to check all ok.
    pub async fn apply(&mut self, cmd: Command<F, LF>) -> ResultGroup<(), AnyError>
    where
        Command<F, LF>: Clone,
    {
        let mut results = Vec::with_capacity(self.workers.len());
        for sender in self.workers.iter_mut() {
            let (upd, rx) = Update::new(cmd.clone());
            match sender.feed(upd).await {
                Ok(_) => match rx.await {
                    Ok(r) => results.push(r),
                    Err(e) => results.push(Err(e.into())),
                },
                Err(e) => results.push(Err(e.into())),
            }
        }
        results.into()
    }
}

impl<F, LF> Manager<F, LF> {
    pub fn new(runtime_config: RuntimeConfig) -> Self {
        let thread_pool = runtime_config
            .thread_pool
            .map(|tn| Box::new(DefaultThreadPool::new(tn)));
        Self {
            runtime_config,
            thread_pool,
            workers: Vec::new(),
        }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.runtime_config
    }
}
