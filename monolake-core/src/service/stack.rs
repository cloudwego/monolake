use super::{layer::FactoryLayer, MakeService, MapTargetService, Service};

pub struct FactoryStack<C, S> {
    config: C,
    inner: S,
}

impl<C> FactoryStack<C, ()> {
    pub fn new(config: C) -> Self {
        FactoryStack { config, inner: () }
    }
}

impl<C, F> FactoryStack<C, F> {
    /// Replace inner with a new factory.
    pub fn replace<NF>(self, factory: NF) -> FactoryStack<C, NF> {
        FactoryStack {
            config: self.config,
            inner: factory,
        }
    }

    /// Push a new factory layer.
    pub fn push<L>(self, layer: L) -> FactoryStack<C, L::Factory>
    where
        L: FactoryLayer<C, F>,
    {
        let inner = layer.layer(&self.config, self.inner);
        FactoryStack {
            config: self.config,
            inner,
        }
    }

    /// Push a new factory of service to map the request type.
    pub fn push_map_target<M: Clone>(self, f: M) -> FactoryStack<C, MapTargetService<F, M>> {
        FactoryStack {
            config: self.config,
            inner: MapTargetService {
                f,
                inner: self.inner,
            },
        }
    }

    /// Check if the stack is a factory of Service<R>.
    pub fn check_make_svc<R>(self) -> Self
    where
        F: MakeService,
        F::Service: Service<R>,
    {
        self
    }

    /// Get the inner factory.
    pub fn into_inner(self) -> F {
        self.inner
    }

    /// Into config and the factory.
    pub fn into_parts(self) -> (C, F) {
        (self.config, self.inner)
    }
}

impl<C, F> FactoryStack<C, F>
where
    F: MakeService,
{
    /// Make a service.
    pub fn make(&self) -> Result<F::Service, F::Error> {
        self.inner.make()
    }
}
