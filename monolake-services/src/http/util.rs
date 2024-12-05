use std::{
    future::Future,
    mem::{ManuallyDrop, MaybeUninit},
    ops::DerefMut,
    pin::Pin,
    task::Poll,
};

use http::{HeaderValue, Request, Response, StatusCode};
use monoio_http::common::body::FixedBody;
use monolake_core::http::{HttpError, HttpHandler, ResponseWithContinue};
use service_async::Service;

union Union<A, B> {
    a: ManuallyDrop<A>,
    b: ManuallyDrop<B>,
}

/// AccompanyPair for http decoder and processor.
/// We have to fill payload when process request
/// since inner logic may read chunked body; also
/// fill payload when process response since we
/// may use the request body stream in response
/// body stream.
pub(crate) struct AccompanyPairBase<F1, F2, FACC, T> {
    main: MaybeUninit<Union<F1, F2>>,
    accompany: FACC,
    accompany_slot: Option<T>,
}

impl<F1, F2, FACC, T> AccompanyPairBase<F1, F2, FACC, T> {
    pub(crate) const fn new(accompany: FACC) -> Self {
        Self {
            main: MaybeUninit::uninit(),
            accompany,
            accompany_slot: None,
        }
    }

    pub(crate) fn stage1(
        mut self: Pin<&mut Self>,
        future1: F1,
    ) -> AccompanyPairS1<F1, F2, FACC, T> {
        unsafe {
            self.as_mut().get_unchecked_mut().main.assume_init_mut().a = ManuallyDrop::new(future1);
        }
        AccompanyPairS1(self)
    }

    pub(crate) fn stage2(
        mut self: Pin<&mut Self>,
        future2: F2,
    ) -> AccompanyPairS2<F1, F2, FACC, T> {
        unsafe {
            self.as_mut().get_unchecked_mut().main.assume_init_mut().b = ManuallyDrop::new(future2);
        }
        AccompanyPairS2(self)
    }

    pub(crate) const fn stage3(self: Pin<&mut Self>) -> AccompanyPairS3<F1, F2, FACC, T> {
        AccompanyPairS3(self)
    }
}

#[repr(transparent)]
pub(crate) struct AccompanyPairS1<'a, F1, F2, FACC, T>(
    Pin<&'a mut AccompanyPairBase<F1, F2, FACC, T>>,
);

impl<F1, F2, FACC, T> Drop for AccompanyPairS1<'_, F1, F2, FACC, T> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.0.as_mut().get_unchecked_mut().main.assume_init_mut().a);
        }
    }
}

impl<F1, F2, FACC, T> Future for AccompanyPairS1<'_, F1, F2, FACC, T>
where
    F1: Future,
    FACC: Future<Output = T>,
{
    type Output = F1::Output;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let this = unsafe { self.0.as_mut().get_unchecked_mut() };
        if this.accompany_slot.is_none()
            && let Poll::Ready(t) = unsafe { Pin::new_unchecked(&mut this.accompany) }.poll(cx)
        {
            this.accompany_slot = Some(t);
        }
        unsafe { Pin::new_unchecked(this.main.assume_init_mut().a.deref_mut()).poll(cx) }
    }
}

#[repr(transparent)]
pub(crate) struct AccompanyPairS2<'a, F1, F2, FACC, T>(
    Pin<&'a mut AccompanyPairBase<F1, F2, FACC, T>>,
);

impl<F1, F2, FACC, T> Drop for AccompanyPairS2<'_, F1, F2, FACC, T> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.0.as_mut().get_unchecked_mut().main.assume_init_mut().b);
        }
    }
}

impl<F1, F2, FACC, T> Future for AccompanyPairS2<'_, F1, F2, FACC, T>
where
    F2: Future,
    FACC: Future<Output = T>,
{
    type Output = F2::Output;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let this = unsafe { self.0.as_mut().get_unchecked_mut() };
        if this.accompany_slot.is_none()
            && let Poll::Ready(t) = unsafe { Pin::new_unchecked(&mut this.accompany) }.poll(cx)
        {
            this.accompany_slot = Some(t);
        }
        unsafe { Pin::new_unchecked(this.main.assume_init_mut().b.deref_mut()).poll(cx) }
    }
}

#[repr(transparent)]
pub(crate) struct AccompanyPairS3<'a, F1, F2, FACC, T>(
    Pin<&'a mut AccompanyPairBase<F1, F2, FACC, T>>,
);

impl<F1, F2, FACC: Future<Output = T>, T> Future for AccompanyPairS3<'_, F1, F2, FACC, T> {
    type Output = FACC::Output;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let this = unsafe { self.0.as_mut().get_unchecked_mut() };
        if let Some(t) = this.accompany_slot.take() {
            return Poll::Ready(t);
        }
        unsafe { Pin::new_unchecked(&mut this.accompany) }.poll(cx)
    }
}

pub(crate) fn generate_response<B: FixedBody>(status_code: StatusCode, close: bool) -> Response<B> {
    let mut resp = Response::builder();
    resp = resp.status(status_code);
    let headers = resp.headers_mut().unwrap();
    if close {
        headers.insert(http::header::CONNECTION, super::CLOSE_VALUE);
    }
    headers.insert(http::header::CONTENT_LENGTH, HeaderValue::from_static("0"));
    resp.body(B::fixed_body(None)).unwrap()
}

pub struct HttpErrorResponder<T>(pub T);
impl<CX, T, B> Service<(Request<B>, CX)> for HttpErrorResponder<T>
where
    T: HttpHandler<CX, B>,
    T::Error: HttpError<T::Body>,
{
    type Response = ResponseWithContinue<T::Body>;
    type Error = T::Error;

    async fn call(&self, (req, cx): (Request<B>, CX)) -> Result<Self::Response, Self::Error> {
        match self.0.handle(req, cx).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                if let Some(r) = e.to_response() {
                    Ok((r, true))
                } else {
                    Err(e)
                }
            }
        }
    }
}
