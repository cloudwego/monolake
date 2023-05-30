use std::convert::Infallible;

use super::MakeService;

#[derive(Clone)]
pub struct CloneFactory<T> {
    svc: T,
}

impl<T> MakeService for CloneFactory<T>
where
    T: Clone,
{
    type Service = T;

    type Error = Infallible;

    fn make_via_ref(&self, _old: Option<&Self::Service>) -> Result<Self::Service, Self::Error> {
        Ok(self.svc.clone())
    }
}
