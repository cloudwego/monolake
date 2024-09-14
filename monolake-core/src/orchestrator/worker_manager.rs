use std::{sync::Arc, thread::JoinHandle};

use futures_channel::{
    mpsc::{channel, Receiver, Sender},
    oneshot::{Receiver as OReceiver, Sender as OSender},
};
use futures_util::SinkExt;
use monoio::{blocking::DefaultThreadPool, utils::bind_to_cpu_set};
use service_async::AsyncMakeService;
use tracing::warn;

use super::{
    Execute, ResultGroup, RuntimeWrapper, ServiceCommand, ServiceCommandTask, ServiceExecutor,
};
use crate::{config::RuntimeConfig, AnyError};

pub type JoinHandlesWithOutput<FNO> = (Vec<(JoinHandle<()>, OSender<()>)>, Vec<FNO>);

/// Orchestrates and manages a fleet of worker threads, each running a [`ServiceExecutor`].
///
/// The [`WorkerManager`] is responsible for:
/// - Spawning and initializing worker threads
/// - Distributing [`ServiceCommand`]s to all workers
/// - Collecting and aggregating results from worker operations
/// - Managing the lifecycle of worker threads
///
/// It acts as the central coordinator in a multi-threaded service deployment system,
/// bridging the gap between the main application thread and individual worker threads.
///
/// # Type Parameters
///
/// * `F`: The type of the service factory used in [`ServiceCommand`]s.
/// * `LF`: The type of the listener factory used in [`ServiceCommand`]s.
///
/// # Fields
///
/// * `runtime_config`: Configuration for the runtime environment of worker threads.
/// * `thread_pool`: An optional thread pool for executing blocking operations.
/// * `workers`: A collection of channels to communicate with individual [`ServiceExecutor`]s.
///
/// # Worker Thread Management
///
/// The manager spawns worker threads based on the `runtime_config`. Each worker thread:
/// - Runs its own [`ServiceExecutor`] instance
/// - Can be optionally bound to a specific CPU core for improved performance
/// - Receives [`ServiceCommand`]s through a dedicated channel
///
/// # Usage
///
/// Typically, a `WorkerManager` is created once at application startup.
///
/// After initialization, [`ServiceCommand`]s can be broadcast to all workers.
///
/// # Thread Safety
///
/// While the `WorkerManager` itself is not thread-safe and should be used from a single
/// thread, it manages communication with multiple worker threads in a thread-safe manner using
/// channels.
pub struct WorkerManager<F, LF> {
    runtime_config: RuntimeConfig,
    thread_pool: Option<Box<DefaultThreadPool>>,
    workers: Vec<Sender<ServiceCommandTask<F, LF>>>,
}

impl<F, LF> WorkerManager<F, LF>
where
    F: Send + 'static,
    LF: Send + 'static,
{
    /// Spawns worker threads asynchronously, each running a [`ServiceExecutor`].
    ///
    /// This method initializes the worker threads based on the `runtime_config` and
    /// returns handles to these threads along with channels to signal their termination.
    ///
    /// # Type Parameters
    ///
    /// * `A`: The type of the argument passed to the service.
    ///
    /// # Returns
    ///
    /// A vector of tuples, each containing:
    /// - A `JoinHandle` for the spawned worker thread
    /// - An `OSender` that can be used to signal the worker to shut down
    #[inline]
    pub fn spawn_workers_async<A>(&mut self) -> Vec<(JoinHandle<()>, OSender<()>)>
    where
        F: AsyncMakeService,
        ServiceCommand<F, LF>: Execute<A, F::Service>,
    {
        self.spawn_workers_inner(
            |mut finish_rx, rx, _worker_id, _pre_f| {
                move |mut runtime: RuntimeWrapper| {
                    let worker_controller = ServiceExecutor::<F::Service>::default();
                    runtime.block_on(async move {
                        worker_controller.run(rx).await;
                        finish_rx.close();
                    });
                }
            },
            |_| (|| (), ()),
        )
        .0
    }
    /// Spawns worker threads with custom initialization functions.
    ///
    /// Similar to `spawn_workers_async`, but allows specifying a custom function
    /// to be executed at the start of each worker thread.
    ///
    /// # Type Parameters
    ///
    /// * `A`: The type of the argument passed to the service.
    /// * `FN`: The type of the function that generates initialization functions and outputs.
    /// * `FNL`: The type of the initialization function.
    /// * `FNO`: The type of the output from the initialization function.
    ///
    /// # Arguments
    ///
    /// * `f`: A function that takes a worker ID and returns a tuple of (initialization function,
    ///   output).
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - A vector of `(JoinHandle, OSender)` pairs for each worker thread.
    /// - A vector of outputs from the initialization functions.
    #[inline]
    pub fn spawn_workers_async_with_fn<A, FN, FNL, FNO>(
        &mut self,
        f: FN,
    ) -> JoinHandlesWithOutput<FNO>
    where
        F: AsyncMakeService,
        ServiceCommand<F, LF>: Execute<A, F::Service>,
        FN: Fn(usize) -> (FNL, FNO),
        FNL: Fn() + Send + 'static,
    {
        self.spawn_workers_inner(
            |mut finish_rx, rx, _worker_id, pre_f| {
                move |mut runtime: RuntimeWrapper| {
                    let worker_controller = ServiceExecutor::<F::Service>::default();
                    runtime.block_on(async move {
                        pre_f();
                        worker_controller.run(rx).await;
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
        S: Fn(OReceiver<()>, Receiver<ServiceCommandTask<F, LF>>, usize, FNL) -> SO,
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
                        // bind thread to cpu core
                        if let Some(cores) = cores {
                            let core = worker_id % cores;
                            if let Err(e) = bind_to_cpu_set([core]) {
                                warn!("bind thread {worker_id} to core {core} failed: {e}");
                            }
                        }
                        f(RuntimeWrapper::new(
                            runtime_config.as_ref(),
                            thread_pool.map(|p| p as Box<_>),
                        ))
                    })
                    .expect("start worker thread {worker_id} failed");
                self.workers.push(tx);
                (handler, finish_tx)
            })
            .collect();
        (out, pre_out)
    }

    /// Dispatches a [`ServiceCommand`] to all managed workers and collects their results.
    ///
    /// This method is a crucial part of worker coordination, enabling synchronized
    /// operations across all worker threads. It demonstrates how the [`WorkerManager`]
    /// orchestrates actions defined by [`ServiceCommand`]s across multiple worker threads.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The [`ServiceCommand`] to be dispatched to all workers.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The service factory type, typically implementing [`AsyncMakeService`].
    /// * `LF` - The listener factory type.
    ///
    /// # Returns
    ///
    /// Returns a [`ResultGroup`] containing the results from all workers. Each result is
    /// either a success (`Ok(())`) or an error (`Err(AnyError)`).
    pub async fn dispatch_service_command(
        &mut self,
        cmd: ServiceCommand<F, LF>,
    ) -> ResultGroup<(), AnyError>
    where
        ServiceCommand<F, LF>: Clone,
    {
        let mut results = Vec::with_capacity(self.workers.len());
        for sender in self.workers.iter_mut() {
            let (upd, rx) = ServiceCommandTask::new(cmd.clone());
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

impl<F, LF> WorkerManager<F, LF> {
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
