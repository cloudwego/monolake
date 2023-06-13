use std::{future::Future, task::Poll};

pin_project_lite::pin_project! {
    /// AccompanyPair for http decoder and processor.
    /// We have to fill payload when process request
    /// since inner logic may read chunked body; also
    /// fill payload when process response since we
    /// may use the request body stream in response
    /// body stream.
    pub(crate) struct AccompanyPair<FMAIN, FACC, T> {
        #[pin]
        main: FMAIN,
        #[pin]
        accompany: FACC,
        accompany_slot: Option<T>
    }
}

pin_project_lite::pin_project! {
    /// Accompany for http decoder and processor.
    pub(crate) struct Accompany<FACC, T> {
        #[pin]
        accompany: FACC,
        accompany_slot: Option<T>
    }
}

impl<FMAIN, FACC, T> Future for AccompanyPair<FMAIN, FACC, T>
where
    FMAIN: Future,
    FACC: Future<Output = T>,
{
    type Output = FMAIN::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if this.accompany_slot.is_none() && let Poll::Ready(t) = this.accompany.poll(cx) {
            *this.accompany_slot = Some(t);
        }
        this.main.poll(cx)
    }
}

impl<FACC, T> Future for Accompany<FACC, T>
where
    FACC: Future<Output = T>,
{
    type Output = T;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if let Some(t) = this.accompany_slot.take() {
            return Poll::Ready(t);
        }
        this.accompany.poll(cx)
    }
}

impl<FMAIN, FACC, T> AccompanyPair<FMAIN, FACC, T> {
    pub(crate) fn new(main: FMAIN, accompany: FACC) -> Self {
        Self {
            main,
            accompany,
            accompany_slot: None,
        }
    }

    pub(crate) fn replace<FMAIN2>(self, main: FMAIN2) -> AccompanyPair<FMAIN2, FACC, T> {
        AccompanyPair {
            main,
            accompany: self.accompany,
            accompany_slot: self.accompany_slot,
        }
    }

    pub(crate) fn into_accompany(self) -> Accompany<FACC, T> {
        Accompany {
            accompany: self.accompany,
            accompany_slot: self.accompany_slot,
        }
    }
}
