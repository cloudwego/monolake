use std::{fmt::Display, future::Future};

use tower_layer::Layer;

pub type ServiceError = anyhow::Error;

pub trait Service<Request>: Clone {
    /// Responses given by the service.
    type Response;
    /// Errors produced by the service.
    type Error: Display;

    /// The future response value.
    type Future<'cx>: Future<Output = Result<Self::Response, Self::Error>>
    where
        Self: 'cx;

    /// Process the request and return the response asynchronously.
    fn call(&self, req: Request) -> Self::Future<'_>;
}

pub trait ServiceLayer<S> {
    type Param: Clone;
    type Layer: Layer<S, Service = Self>;

    fn layer(param: Self::Param) -> Self::Layer;
}
