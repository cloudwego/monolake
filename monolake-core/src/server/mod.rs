use std::{cell::UnsafeCell, collections::HashMap, fmt::Debug, io, rc::Rc, sync::Arc};

use anyhow::anyhow;
use futures_channel::{
    mpsc::{channel, Receiver, Sender},
    oneshot::{channel as ochannel, Receiver as OReceiver, Sender as OSender},
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use monoio::{blocking::DefaultThreadPool, io::stream::Stream, utils::bind_to_cpu_set};
use service_async::{AsyncMakeService, MakeService, Service};
use tracing::{debug, error, info, warn};

use self::runtime::RuntimeWrapper;
use crate::{bail_into, config::RuntimeConfig, AnyError};

mod runtime;

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
    pub fn spawn_workers<A>(
        &mut self,
    ) -> Vec<(
        std::thread::JoinHandle<()>,
        futures_channel::oneshot::Sender<()>,
    )>
    where
        F: MakeService,
        Command<F, LF>: Execute<A, F::Service>,
    {
        self.spawn_workers_inner(|mut finish_rx, rx| {
            move |mut runtime: RuntimeWrapper| {
                let worker_controller = WorkerController::<F::Service>::default();
                runtime.block_on(async move {
                    worker_controller.run_controller(rx).await;
                    finish_rx.close();
                });
            }
        })
    }

    #[inline]
    pub fn spawn_workers_async<A>(
        &mut self,
    ) -> Vec<(
        std::thread::JoinHandle<()>,
        futures_channel::oneshot::Sender<()>,
    )>
    where
        F: AsyncMakeService,
        Command<F, LF>: AsyncExecute<A, F::Service>,
    {
        self.spawn_workers_inner(|mut finish_rx, rx| {
            move |mut runtime: RuntimeWrapper| {
                let worker_controller = WorkerController::<F::Service>::default();
                runtime.block_on(async move {
                    worker_controller.run_controller_async(rx).await;
                    finish_rx.close();
                });
            }
        })
    }

    /// Start workers according to runtime config.
    /// Threads JoinHandle will be returned and each factory Sender will
    /// be saved for config updating.
    pub fn spawn_workers_inner<S, SO>(
        &mut self,
        fn_lambda: S,
    ) -> Vec<(
        std::thread::JoinHandle<()>,
        futures_channel::oneshot::Sender<()>,
    )>
    where
        S: Fn(
            futures_channel::oneshot::Receiver<()>,
            futures_channel::mpsc::Receiver<Update<F, LF>>,
        ) -> SO,
        SO: FnOnce(RuntimeWrapper) + Send + 'static,
    {
        let cores = if self.runtime_config.cpu_affinity {
            std::thread::available_parallelism().ok()
        } else {
            None
        };

        let runtime_config = Arc::new(self.runtime_config.clone());
        (0..self.runtime_config.worker_threads)
            .map(|worker_id| {
                let thread_pool = self.thread_pool.clone();
                let (tx, rx) = channel(128);
                let runtime_config = runtime_config.clone();
                let (finish_tx, finish_rx) = futures_channel::oneshot::channel::<()>();
                let f = fn_lambda(finish_rx, rx);
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
            .collect()
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

pub struct ResultGroup<T, E>(Vec<Result<T, E>>);

impl<T, E> From<Vec<Result<T, E>>> for ResultGroup<T, E> {
    fn from(value: Vec<Result<T, E>>) -> Self {
        Self(value)
    }
}

impl<T, E> From<ResultGroup<T, E>> for Vec<Result<T, E>> {
    fn from(value: ResultGroup<T, E>) -> Self {
        value.0
    }
}

impl<E> ResultGroup<(), E> {
    pub fn err(self) -> Result<(), E> {
        for r in self.0.into_iter() {
            r?;
        }
        Ok(())
    }
}

/// WorkerController is holden by worker threads, it saved every sites' service.
// TODO: make up a better name.
pub struct WorkerController<S> {
    sites: Rc<UnsafeCell<HashMap<Arc<String>, SiteHandler<S>>>>,
}

impl<S> Default for WorkerController<S> {
    fn default() -> Self {
        Self {
            sites: Rc::new(UnsafeCell::new(HashMap::new())),
        }
    }
}

pub struct SiteHandler<S> {
    handler_slot: HandlerSlot<S>,
    two_stage_handler_slot: UnsafeCell<Option<S>>,
    _stop: OReceiver<()>,
}

impl<S> SiteHandler<S> {
    pub fn new(handler_slot: HandlerSlot<S>) -> (Self, OSender<()>) {
        let (tx, rx) = ochannel();
        (
            Self {
                handler_slot,
                two_stage_handler_slot: UnsafeCell::new(None),
                _stop: rx,
            },
            tx,
        )
    }
}

pub struct HandlerSlot<S>(Rc<UnsafeCell<Rc<S>>>);

impl<S> Clone for HandlerSlot<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> From<Rc<S>> for HandlerSlot<S> {
    fn from(value: Rc<S>) -> Self {
        Self(Rc::new(UnsafeCell::new(value)))
    }
}

impl<S> From<Rc<UnsafeCell<Rc<S>>>> for HandlerSlot<S> {
    fn from(value: Rc<UnsafeCell<Rc<S>>>) -> Self {
        Self(value)
    }
}

impl<S> HandlerSlot<S> {
    pub fn update_svc(&self, shared_svc: Rc<S>) {
        unsafe { *self.0.get() = shared_svc };
    }

    pub fn get_svc(&self) -> Rc<S> {
        unsafe { &*self.0.get() }.clone()
    }
}

/// It should be cheap to clone.
#[allow(dead_code)]
#[derive(Clone)]
pub enum Command<F, LF> {
    Add(Arc<String>, F, LF),
    Update(Arc<String>, F),
    Remove(Arc<String>),
    TwoStageCreate(Arc<String>, F),
    TwoStageApply(Arc<String>),
    TwoStageAbort(Arc<String>),
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

pub trait AsyncExecute<A, S> {
    fn async_execute(
        self,
        controller: &WorkerController<S>,
    ) -> impl std::future::Future<Output = Result<(), AnyError>>;
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
                let sites = unsafe { &mut *controller.sites.get() };
                match sites.get(&name).map(|sh| sh.handler_slot.clone()) {
                    Some(svc_slot) => {
                        let svc = factory
                            .make_via_ref(Some(&svc_slot.get_svc()))
                            .map_err(|e| anyhow!("build service fail {e:?}"))?;
                        svc_slot.update_svc(Rc::new(svc));
                        Ok(())
                    }
                    None => bail_into!("site {name} not exist"),
                }
            }
            Command::TwoStageCreate(name, factory) => {
                let sites = unsafe { &mut *controller.sites.get() };
                let Some(sh) = sites.get(&name) else {
                    bail_into!("site {name} not exist");
                };

                let current_svc = sh.handler_slot.clone();
                let svc = factory
                    .make_via_ref(Some(&current_svc.get_svc()))
                    .map_err(|e| anyhow!("build service fail {e:?}"))?;
                unsafe { *sh.two_stage_handler_slot.get() = Some(svc) };
                Ok(())
            }
            Command::TwoStageApply(name) => {
                let sites = unsafe { &mut *controller.sites.get() };
                let Some(sh) = sites.get(&name) else {
                    bail_into!("site {name} not exist");
                };

                let svc = unsafe { (*sh.two_stage_handler_slot.get()).take() }
                    .ok_or_else(|| anyhow!("two stage service not exist"))?;
                sh.handler_slot.update_svc(Rc::new(svc));
                Ok(())
            }
            Command::TwoStageAbort(name) => {
                let sites = unsafe { &mut *controller.sites.get() };
                let Some(sh) = sites.get(&name) else {
                    bail_into!("site {name} not exist");
                };
                unsafe { (*sh.two_stage_handler_slot.get()) = None };
                Ok(())
            }
            Command::Add(name, factory, listener_factory) => {
                // TODO: make sure the named service has not been started
                let listener = match listener_factory.make() {
                    Ok(l) => l,
                    Err(e) => bail_into!("create listener fail for site {name}: {e:?}"),
                };
                let svc = match factory.make() {
                    Ok(l) => l,
                    Err(e) => bail_into!("create service fail for site {name}: {e:?}"),
                };
                let new_slot = HandlerSlot::from(Rc::new(svc));
                let (site_handler, stop) = SiteHandler::new(new_slot.clone());
                {
                    let sites = unsafe { &mut *controller.sites.get() };
                    sites.insert(name, site_handler);
                }
                monoio::spawn(serve(listener, new_slot, stop));
                Ok(())
            }
            Command::Remove(name) => {
                let sites = unsafe { &mut *controller.sites.get() };
                if sites.remove(&name).is_none() {
                    bail_into!("site {name} not exist");
                }
                Ok(())
            }
        }
    }
}

impl<F, LF, A, S> AsyncExecute<A, S> for Command<F, LF>
where
    F: AsyncMakeService<Service = S>,
    F::Error: Debug,
    LF: AsyncMakeService,
    LF::Service: Stream<Item = io::Result<A>> + 'static,
    LF::Error: Debug,
    S: Service<A> + 'static,
    S::Error: Debug,
    A: 'static,
{
    async fn async_execute(self, controller: &WorkerController<S>) -> Result<(), AnyError> {
        match self {
            Command::Update(name, factory) => {
                let sites = unsafe { &mut *controller.sites.get() };
                match sites.get(&name).map(|sh| sh.handler_slot.clone()) {
                    Some(svc_slot) => {
                        let svc = factory
                            .make_via_ref(Some(&svc_slot.get_svc()))
                            .await
                            .map_err(|e| anyhow!("build service fail {e:?}"))?;
                        svc_slot.update_svc(Rc::new(svc));
                        Ok(())
                    }
                    None => bail_into!("site {name} not exist"),
                }
            }
            Command::TwoStageCreate(name, factory) => {
                let sites = unsafe { &mut *controller.sites.get() };
                let Some(sh) = sites.get(&name) else {
                    bail_into!("site {name} not exist");
                };

                let current_svc = sh.handler_slot.clone();
                let svc = factory
                    .make_via_ref(Some(&current_svc.get_svc()))
                    .await
                    .map_err(|e| anyhow!("build service fail {e:?}"))?;
                unsafe { *sh.two_stage_handler_slot.get() = Some(svc) };
                Ok(())
            }
            Command::TwoStageApply(name) => {
                let sites = unsafe { &mut *controller.sites.get() };
                let Some(sh) = sites.get(&name) else {
                    bail_into!("site {name} not exist");
                };

                let svc = unsafe { (*sh.two_stage_handler_slot.get()).take() }
                    .ok_or_else(|| anyhow!("two stage service not exist"))?;
                sh.handler_slot.update_svc(Rc::new(svc));
                Ok(())
            }
            Command::TwoStageAbort(name) => {
                let sites = unsafe { &mut *controller.sites.get() };
                let Some(sh) = sites.get(&name) else {
                    bail_into!("site {name} not exist");
                };
                unsafe { (*sh.two_stage_handler_slot.get()) = None };
                Ok(())
            }
            Command::Add(name, factory, listener_factory) => {
                // TODO: make sure the named service has not been started
                let listener = match listener_factory.make().await {
                    Ok(l) => l,
                    Err(e) => bail_into!("create listener fail for site {name}: {e:?}"),
                };
                let svc = match factory.make().await {
                    Ok(l) => l,
                    Err(e) => bail_into!("create service fail for site {name}: {e:?}"),
                };
                let new_slot = HandlerSlot::from(Rc::new(svc));
                let (site_handler, stop) = SiteHandler::new(new_slot.clone());
                {
                    let sites = unsafe { &mut *controller.sites.get() };
                    sites.insert(name, site_handler);
                }
                monoio::spawn(serve(listener, new_slot, stop));
                Ok(())
            }
            Command::Remove(name) => {
                let sites = unsafe { &mut *controller.sites.get() };
                if sites.remove(&name).is_none() {
                    bail_into!("site {name} not exist");
                }
                Ok(())
            }
        }
    }
}

impl<S> WorkerController<S> {
    pub async fn run_controller_async<F, LF, A>(&self, mut rx: Receiver<Update<F, LF>>)
    where
        Command<F, LF>: AsyncExecute<A, S>,
    {
        while let Some(upd) = rx.next().await {
            if let Err(e) = upd.result.send(upd.cmd.async_execute(self).await) {
                error!("unable to send back result: {e:?}");
            }
        }
    }

    pub async fn run_controller<F, LF, A>(&self, mut rx: Receiver<Update<F, LF>>)
    where
        Command<F, LF>: Execute<A, S>,
    {
        while let Some(upd) = rx.next().await {
            if let Err(e) = upd.result.send(upd.cmd.execute(self)) {
                error!("unable to send back result: {e:?}");
            }
        }
    }
}

pub async fn serve<S, Svc, A>(mut listener: S, handler: HandlerSlot<Svc>, mut stop: OSender<()>)
where
    S: Stream<Item = io::Result<A>> + 'static,
    Svc: Service<A> + 'static,
    Svc::Error: Debug,
    A: 'static,
{
    let mut cancellation = stop.cancellation();
    loop {
        monoio::select! {
            _ = &mut cancellation => {
                info!("server is notified to stop");
                break;
            }
            accept_opt = listener.next() => {
                let accept = match accept_opt {
                    Some(accept) => accept,
                    None => {
                        info!("listener is closed, serve stopped");
                        return;
                    }
                };
                match accept {
                    Ok(accept) => {
                        let svc = handler.get_svc();
                        monoio::spawn(async move {
                            match svc.call(accept).await {
                                Ok(_) => {
                                    debug!("Connection complete");
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
    }
}
