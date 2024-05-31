use std::fmt::Debug;

use futures_channel::oneshot::Sender as OSender;
use monoio::io::stream::Stream;
use service_async::Service;
use tracing::{debug, error, info, warn};

use self::runtime::RuntimeWrapper;

mod manager;
mod runtime;
mod site;

pub use manager::{JoinHandlesWithOutput, Manager};
pub use site::{Command, Execute, HandlerSlot, SiteHandler, Update, WorkerController};

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

pub async fn serve<S, Svc, A, E>(mut listener: S, handler: HandlerSlot<Svc>, mut stop: OSender<()>)
where
    S: Stream<Item = Result<A, E>> + 'static,
    E: Debug,
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
