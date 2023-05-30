use std::{cell::UnsafeCell, collections::HashMap, fmt::Debug, io, rc::Rc, sync::Arc};

use anyhow::{anyhow, bail};
use futures_channel::{
    mpsc::{channel, Receiver, Sender},
    oneshot::{channel as ochannel, Receiver as OReceiver, Sender as OSender},
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tracing::{error, info, warn};
use monoio::{io::stream::Stream, utils::bind_to_cpu_set};
use monolake_core::{
    config::RuntimeConfig,
    service::{MakeService, Service},
};
use monolake_services::AnyError;

use self::runtime::RuntimeWrapper;

mod runtime;

pub struct Manager<F, LF> {
    runtime_config: RuntimeConfig,
    workers: Vec<Sender<Update<F, LF>>>,
}

impl<F, LF> Manager<F, LF>
where
    F: Send + 'static,
    LF: Send + 'static,
    F: MakeService,
{
    pub fn spawn_workers<A>(&mut self) -> Vec<std::thread::JoinHandle<()>>
    where
        Command<F, LF>: Execute<A, F::Service>,
    {
        let cores = if self.runtime_config.cpu_affinity {
            std::thread::available_parallelism().ok()
        } else {
            None
        };

        let runtime_config = Arc::new(self.runtime_config.clone());
        (0..self.runtime_config.worker_threads)
            .map(|worker_id| {
                let (tx, rx) = channel(128);
                let runtime_config = runtime_config.clone();
                let handler = std::thread::Builder::new()
                    .name(format!("monolake-worker-{worker_id}"))
                    .spawn(move || {
                        let worker_controller = WorkerController::<F::Service>::default();
                        if let Some(cores) = cores {
                            let core = worker_id % cores;
                            if let Err(e) = bind_to_cpu_set([core]) {
                                tracing::warn!("bind thread {worker_id} to core {core} failed: {e}");
                            }
                        }
                        let mut runtime = RuntimeWrapper::from(runtime_config.as_ref());
                        runtime.block_on(worker_controller.run_controller(rx));
                    })
                    .expect("start worker thread {worker_id} failed");
                self.workers.push(tx);
                handler
            })
            .collect()
    }

    pub async fn apply(&mut self, cmd: Command<F, LF>) -> Vec<Result<(), AnyError>>
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
        results
    }
}

impl<F, LF> Manager<F, LF> {
    pub fn new(runtime_config: RuntimeConfig) -> Self {
        Self {
            runtime_config,
            workers: Vec::new(),
        }
    }
}

pub struct WorkerController<S> {
    sites: Rc<UnsafeCell<HashMap<String, Rc<S>>>>,
}

impl<S> Default for WorkerController<S> {
    fn default() -> Self {
        Self {
            sites: Rc::new(UnsafeCell::new(HashMap::new())),
        }
    }
}

/// It should be cheap to clone.
#[derive(Clone)]
pub enum Command<F, LF> {
    Update(String, F),
    Add(String, F, LF),
    Del(String),
}

pub struct Update<F, LF> {
    cmd: Command<F, LF>,
    result: OSender<Result<(), AnyError>>,
}

impl<F, LF> Update<F, LF> {
    pub fn new(cmd: Command<F, LF>) -> (Self, OReceiver<Result<(), AnyError>>) {
        let (tx, rx) = ochannel();
        (Self { cmd, result: tx }, rx)
    }
}

pub trait Execute<A, S> {
    fn execute(self, controller: &WorkerController<S>) -> Result<(), AnyError>;
}

impl<F, LF, A, S> Execute<A, S> for Command<F, LF>
where
    F: MakeService<Service = S>,
    F::Error: Debug,
    LF: MakeService,
    LF::Service: Stream<Item = io::Result<A>> + 'static,
    LF::Error: Debug,
    S: Service<A> + 'static,
    S::Error: Debug,
    A: 'static,
{
    fn execute(self, controller: &WorkerController<S>) -> Result<(), AnyError> {
        match self {
            Command::Update(name, factory) => {
                match {
                    let sites = unsafe { &mut *controller.sites.get() };
                    sites.get(&name).cloned()
                } {
                    Some(old_svc) => {
                        let svc = factory
                            .make_via_ref(Some(&old_svc))
                            .map_err(|e| anyhow!("build service fail {e:?}"))?;
                        let sites = unsafe { &mut *controller.sites.get() };
                        sites.insert(name, Rc::new(svc));
                        Ok(())
                    }
                    None => bail!("site {name} not exist"),
                }
            }
            Command::Add(name, factory, listener_factory) => {
                // TODO: check the Name has not been started
                let listener = match listener_factory.make() {
                    Ok(l) => l,
                    Err(e) => bail!("create listener fail for site {name}: {e:?}"),
                };
                let svc = match factory.make() {
                    Ok(l) => l,
                    Err(e) => bail!("create service fail for site {name}: {e:?}"),
                };
                monoio::spawn(serve(listener, Rc::new(svc)));
                Ok(())
            }
            Command::Del(name) => {
                // TODO: make the server stop!
                let sites = unsafe { &mut *controller.sites.get() };
                if sites.remove(&name).is_none() {
                    bail!("site {name} not exist");
                }
                Ok(())
            }
        }
    }
}

impl<S> WorkerController<S> {
    pub async fn run_controller<F, LF, A>(&self, mut rx: Receiver<Update<F, LF>>)
    where
        Command<F, LF>: Execute<A, S>,
    {
        tracing::info!("worker controller started");
        while let Some(upd) = rx.next().await {
            tracing::info!("got an update");
            if let Err(e) = upd.result.send(upd.cmd.execute(self)) {
                tracing::error!("unable to send back result: {e:?}");
            }
        }
        tracing::info!("worker coltroller exit");
    }
}

pub async fn serve<S, Svc, A>(mut listener: S, handler: Rc<Svc>)
where
    S: Stream<Item = io::Result<A>> + 'static,
    Svc: Service<A> + 'static,
    Svc::Error: Debug,
    A: 'static,
{
    while let Some(accept) = listener.next().await {
        match accept {
            Ok(accept) => {
                let svc = handler.clone();
                monoio::spawn(async move {
                    match svc.call(accept).await {
                        Ok(_) => {
                            info!("Connection complete");
                        }
                        Err(e) => {
                            error!("Connection error: {e:?}");
                        }
                    }
                });
            }
            Err(e) => warn!("Accept connection failed: {e:?}"),
        }
    }
}
