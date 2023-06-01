use std::{cell::UnsafeCell, fmt::Debug, future::Future, rc::Rc};

use async_channel::Receiver;
use monoio::io::stream::Stream;
use monolake_core::service::{MakeService, Service};

#[derive(Clone)]
struct ReloadableServer<S> {
    // Use UnsafeCell to make it can be replaced.
    inner: Rc<UnsafeCell<Rc<S>>>,
}

impl<L, S, SE, C> Service<L> for ReloadableServer<S>
where
    L: Stream<Item = Result<C, SE>>,
    S: Service<C> + 'static,
    S::Error: Debug,
    SE: Debug,
    C: 'static,
{
    type Response = ();
    type Error = SE;
    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + 'cx
    where
        Self: 'cx,
        L: 'cx;

    fn call(&self, mut listener: L) -> Self::Future<'_> {
        async move {
            while let Some(accept) = listener.next().await {
                match accept {
                    Ok(accept) => {
                        // # Safety
                        // We can make sure the Service is not Sync, so
                        // only current thread can use it. The borrowed
                        // one will only be used in synchronized logic.
                        let svc = unsafe { &*self.inner.get() }.clone();
                        monoio::spawn(async move {
                            match svc.call(accept).await {
                                Ok(_) => {
                                    tracing::info!("Connection complete");
                                }
                                Err(e) => {
                                    tracing::error!("Connection handling error: {e:?}");
                                }
                            }
                        });
                    }
                    Err(e) => tracing::warn!("Accept connection failed: {e:?}"),
                }
            }
            tracing::info!("Listener complete");
            Ok(())
        }
    }
}

impl<S> ReloadableServer<S> {
    // The task will exit when Sender dropped.
    pub async fn reload_background<T>(&self, recvier: Receiver<T>)
    where
        T: MakeService<Service = S>,
        T::Error: Debug,
    {
        while let Ok(new_factory) = recvier.recv().await {
            let old = unsafe { &*self.inner.get() }.clone();
            match new_factory.make_via_ref(Some(&old)) {
                Ok(new_svc) => unsafe { *self.inner.get() = Rc::new(new_svc) },
                Err(err) => {
                    tracing::error!("Fail to build the service chain: {err:?}");
                },
            }
        }
        tracing::info!("Reload channel closed, reload task exit.");
    }
}
