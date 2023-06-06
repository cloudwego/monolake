use std::{future::Future, task::Poll};

pin_project_lite::pin_project! {
    /// MaybeDoubleFuture for http decoder and processor.
    #[project = EnumProj]
    pub(crate) enum MaybeDoubleFuture<FA, FB, T> {
        Single{#[pin] fut: FA},
        Double{
            #[pin] future_a: FA,
            #[pin] future_b: FB,
            slot_a: Option<T>,
            ready_b: bool
        },
    }
}

impl<FA, FB, T> Future for MaybeDoubleFuture<FA, FB, T>
where
    FA: Future<Output = T>,
    FB: Future,
{
    type Output = T;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            EnumProj::Single { fut } => fut.poll(cx),
            EnumProj::Double {
                future_a,
                future_b,
                slot_a,
                ready_b,
            } => {
                // try poll future_b if not ready
                if !*ready_b && matches!(future_b.poll(cx), Poll::Ready(_)) {
                    *ready_b = true;
                }
                // poll future_a if not ready
                if slot_a.is_none() {
                    if let Poll::Ready(t) = future_a.poll(cx) {
                        *slot_a = Some(t);
                    }
                }
                // if future_b is not ready, return pending
                if !*ready_b {
                    return Poll::Pending;
                }
                // now future_b is ready, check a
                match slot_a.take() {
                    Some(t) => Poll::Ready(t),
                    None => Poll::Pending,
                }
            }
        }
    }
}

impl<FA, FB, T> MaybeDoubleFuture<FA, FB, T>
where
    FA: Future<Output = T>,
{
    pub(crate) fn new(future_a: FA, future_b: Option<FB>) -> MaybeDoubleFuture<FA, FB, T> {
        if let Some(future_b) = future_b {
            MaybeDoubleFuture::Double {
                future_a,
                future_b,
                slot_a: None,
                ready_b: false,
            }
        } else {
            MaybeDoubleFuture::Single { fut: future_a }
        }
    }
}
