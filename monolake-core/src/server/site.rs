use std::{cell::UnsafeCell, collections::HashMap, fmt::Debug, io, rc::Rc, sync::Arc};

use futures_channel::{
    mpsc::Receiver,
    oneshot::{channel as ochannel, Receiver as OReceiver, Sender as OSender},
};
use futures_util::stream::StreamExt;
use monoio::io::stream::Stream;
use service_async::{AsyncMakeService, Service};
use tracing::error;

use super::serve;
use crate::AnyError;

/// WorkerController is holden by worker threads, it saved every sites' service.
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

enum WorkerCtlNotExist {
    Site,
    Preparation,
    PreviousHandler,
}

impl<S> WorkerController<S> {
    // Lookup and clone service.
    fn get_svc(&self, name: &Arc<String>) -> Option<Rc<S>> {
        let sites = unsafe { &*self.sites.get() };
        sites.get(name).and_then(|s| s.get_svc())
    }

    // Set parpart slot with given S.
    fn set_prepare(&self, name: Arc<String>, prepare: S) {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites.entry(name).or_insert_with(SiteHandler::new);
        let prepare_slot = unsafe { &mut *sh.prepare_slot.get() };
        *prepare_slot = Some(prepare);
    }

    // Apply prepare to handler slot(must not be empty).
    fn apply_prepare_update(&self, name: &Arc<String>) -> Result<(), WorkerCtlNotExist> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites.get_mut(name).ok_or(WorkerCtlNotExist::Site)?;

        let hdr = sh
            .handler
            .as_mut()
            .ok_or(WorkerCtlNotExist::PreviousHandler)?;
        let prepare_slot = unsafe { &mut *sh.prepare_slot.get() };
        let prepare = prepare_slot.take().ok_or(WorkerCtlNotExist::Preparation)?;

        hdr.slot.update_svc(Rc::new(prepare));
        Ok(())
    }

    // Apply prepare to handler slot(must be empty).
    fn apply_prepare_create(
        &self,
        name: &Arc<String>,
    ) -> Result<(HandlerSlot<S>, OSender<()>), WorkerCtlNotExist> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites.get_mut(name).ok_or(WorkerCtlNotExist::Site)?;
        let prepare_slot = unsafe { &mut *sh.prepare_slot.get() };
        let prepare = prepare_slot.take().ok_or(WorkerCtlNotExist::Preparation)?;

        let (new_site, stop) = Handler::create(prepare);
        let handler_slot = new_site.slot.clone();
        sh.handler = Some(new_site);
        Ok((handler_slot, stop))
    }

    // Remove site.
    fn remove(&self, name: &Arc<String>) -> Result<(), WorkerCtlNotExist> {
        let sites = unsafe { &mut *self.sites.get() };
        if sites.remove(name).is_none() {
            Err(WorkerCtlNotExist::Site)
        } else {
            Ok(())
        }
    }

    fn unprepare(&self, name: &Arc<String>) -> Result<(), WorkerCtlNotExist> {
        let sites = unsafe { &mut *self.sites.get() };
        let sh = sites.get_mut(name).ok_or(WorkerCtlNotExist::Site)?;
        let prepare_slot = unsafe { &mut *sh.prepare_slot.get() };
        *prepare_slot = None;
        Ok(())
    }
}

pub struct SiteHandler<S> {
    handler: Option<Handler<S>>,
    prepare_slot: UnsafeCell<Option<S>>,
}

struct Handler<S> {
    slot: HandlerSlot<S>,
    _stop: OReceiver<()>,
}

impl<S> SiteHandler<S> {
    const fn new() -> Self {
        Self {
            handler: None,
            prepare_slot: UnsafeCell::new(None),
        }
    }

    fn get_svc(&self) -> Option<Rc<S>> {
        self.handler.as_ref().map(|h| h.slot.get_svc())
    }
}

impl<S> Handler<S> {
    fn create(handler: S) -> (Self, OSender<()>) {
        let (tx, rx) = ochannel();
        (
            Self {
                slot: HandlerSlot::from(Rc::new(handler)),
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
    Prepare(Arc<String>, F),
    ApplyUpdate(Arc<String>),
    ApplyCreate(Arc<String>, LF),
    Init(Arc<String>, F, LF),
    Abort(Arc<String>),
    Remove(Arc<String>),
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError<SE, LE> {
    #[error("build service error: {0:?}")]
    BuildService(SE),
    #[error("build listener error: {0:?}")]
    BuildListener(LE),
    #[error("site not exist")]
    SiteNotExist,
    #[error("preparation not exist")]
    PreparationNotExist,
    #[error("previous handler not exist")]
    PreviousHandlerNotExist,
}

impl<SE, LE> From<WorkerCtlNotExist> for CommandError<SE, LE> {
    fn from(value: WorkerCtlNotExist) -> Self {
        match value {
            WorkerCtlNotExist::Site => Self::SiteNotExist,
            WorkerCtlNotExist::Preparation => Self::PreparationNotExist,
            WorkerCtlNotExist::PreviousHandler => Self::PreviousHandlerNotExist,
        }
    }
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
    type Error: Into<AnyError>;
    fn execute(
        self,
        controller: &WorkerController<S>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;
}

impl<F, LF, A, S> Execute<A, S> for Command<F, LF>
where
    F: AsyncMakeService<Service = S>,
    F::Error: Debug + Send + Sync + 'static,
    LF: AsyncMakeService,
    LF::Service: Stream<Item = io::Result<A>> + 'static,
    LF::Error: Debug + Send + Sync + 'static,
    S: Service<A> + 'static,
    S::Error: Debug,
    A: 'static,
{
    type Error = CommandError<F::Error, LF::Error>;
    async fn execute(self, controller: &WorkerController<S>) -> Result<(), Self::Error> {
        match self {
            Command::Prepare(name, factory) => {
                let current_svc = controller.get_svc(&name);
                let svc = factory
                    .make_via_ref(current_svc.as_deref())
                    .await
                    .map_err(CommandError::BuildService)?;
                controller.set_prepare(name, svc);
                Ok(())
            }
            Command::ApplyUpdate(name) => {
                controller.apply_prepare_update(&name)?;
                Ok(())
            }
            Command::ApplyCreate(name, listener_factory) => {
                let listener = listener_factory
                    .make()
                    .await
                    .map_err(CommandError::BuildListener)?;
                let (hdr, stop) = controller.apply_prepare_create(&name)?;
                monoio::spawn(serve(listener, hdr, stop));
                Ok(())
            }
            Command::Init(name, factory, listener_factory) => {
                let svc = factory.make().await.map_err(CommandError::BuildService)?;
                let listener = listener_factory
                    .make()
                    .await
                    .map_err(CommandError::BuildListener)?;
                controller.set_prepare(name.clone(), svc);
                let (hdr, stop) = controller.apply_prepare_create(&name)?;
                monoio::spawn(serve(listener, hdr, stop));
                Ok(())
            }
            Command::Abort(name) => {
                controller.unprepare(&name)?;
                Ok(())
            }
            Command::Remove(name) => {
                controller.remove(&name)?;
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
        while let Some(upd) = rx.next().await {
            if let Err(e) = upd
                .result
                .send(upd.cmd.execute(self).await.map_err(Into::into))
            {
                error!("unable to send back result: {e:?}");
            }
        }
    }
}
