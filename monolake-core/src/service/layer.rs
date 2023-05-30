use std::marker::PhantomData;

pub trait FactoryLayer<C, F> {
    type Factory;
    fn layer(&self, config: &C, inner: F) -> Self::Factory;
}

pub fn layer_fn<C, F, FN, O>(f: FN) -> LayerFn<C, FN>
where
    FN: Fn(&C, F) -> O,
{
    LayerFn {
        f,
        marker: PhantomData,
    }
}

pub struct LayerFn<C, FN> {
    f: FN,
    marker: PhantomData<fn(C)>,
}

impl<C, F, FN, O> FactoryLayer<C, F> for LayerFn<C, FN>
where
    FN: Fn(&C, F) -> O,
{
    type Factory = O;

    fn layer(&self, config: &C, inner: F) -> Self::Factory {
        (self.f)(config, inner)
    }
}
