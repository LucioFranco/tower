/// Builder types to compose layers and services
mod service;

pub use self::service::{MakerFuture, ServiceBuilderMaker};

use never::Never;
use std::marker::PhantomData;
use tower_layer::{util::Chain, Layer, LayerExt};
use tower_service::Service;
use tower_service_util::MakeService;

pub(super) type Error = Box<::std::error::Error + Send + Sync>;

/// `ServiceBuilder` provides a [builder-like interface](https://doc.rust-lang.org/1.0.0/style/ownership/builders.html) for composing Layers and a connection, where the latter is modeled by
///  a `MakeService`. The builder produces either a new `Service` or `MakeService`,
///  depending on whether `build_svc` or `build_maker` is called.
///
/// # Services and MakeServices
///
/// - A [`Service`](tower_service::Service) is a trait representing an asynchronous
///   function of a request to a response. It is similar to
///   `async fn(Request) -> Result<Response, Error>`.
/// - A [`MakeService`](tower_service_util::MakeService) is a trait creating specific
///   instances of a `Service`
///
/// # Service
///
/// A `Service` is typically bound to a single transport, such as a TCP connection.
/// It defines how _all_ inbound or outbound requests are handled by that connection.
///
/// # MakeService
///
/// Since a `Service` is bound to a single connection, a `MakeService` allows for the
/// creation of _new_ `Service`s that'll be bound to _different_ different connections.
/// This is useful for servers, as they require the ability to accept new connections.
///
/// Resources that need to be shared by all `Service`s can be put into a
/// `MakeService`, and then passed to individual `Service`s when `build_maker`
/// is called.
///
/// # Examples
///
/// A MakeService stack with a single layer:
///
/// ```rust
/// # extern crate tower;
/// # extern crate tower_in_flight_limit;
/// # extern crate futures;
/// # extern crate void;
/// # use void::Void;
/// # use tower::Service;
/// # use tower::builder::ServiceBuilder;
/// # use tower_in_flight_limit::InFlightLimitLayer;
/// # use futures::{Poll, future::{self, FutureResult}};
/// # #[derive(Debug)]
/// # struct MyMakeService;
/// # impl Service<()> for MyMakeService {
/// #    type Response = MyService;
/// #    type Error = Void;
/// #    type Future = FutureResult<Self::Response, Self::Error>;
/// #    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
/// #        Ok(().into())
/// #    }
/// #    fn call(&mut self, _: ()) -> Self::Future {
/// #        future::ok(MyService)
/// #    }
/// # }
/// # #[derive(Debug)]
/// # struct MyService;
/// # impl Service<()> for MyService {
/// #    type Response = ();
/// #    type Error = Void;
/// #    type Future = FutureResult<Self::Response, Self::Error>;
/// #    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
/// #        Ok(().into())
/// #    }
/// #    fn call(&mut self, _: ()) -> Self::Future {
/// #        future::ok(())
/// #    }
/// # }
/// ServiceBuilder::new()
///     .chain(InFlightLimitLayer::new(5))
///     .build_maker(MyMakeService);
/// ```
///
/// A `Service` stack with a single layer:
///
/// ```
/// # extern crate tower;
/// # extern crate tower_in_flight_limit;
/// # extern crate futures;
/// # extern crate void;
/// # use void::Void;
/// # use tower::Service;
/// # use tower::builder::ServiceBuilder;
/// # use tower_in_flight_limit::InFlightLimitLayer;
/// # use futures::{Poll, future::{self, FutureResult}};
/// # #[derive(Debug)]
/// # struct MyService;
/// # impl Service<()> for MyService {
/// #    type Response = ();
/// #    type Error = Void;
/// #    type Future = FutureResult<Self::Response, Self::Error>;
/// #    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
/// #        Ok(().into())
/// #    }
/// #    fn call(&mut self, _: ()) -> Self::Future {
/// #        future::ok(())
/// #    }
/// # }
/// ServiceBuilder::new()
///     .chain(InFlightLimitLayer::new(5))
///     .build_svc(MyService);
/// ```
///
/// A `Service` stack with _multiple_ layers that contain rate limiting, in-flight request limits,
/// and a channel-backed, clonable `Service`:
///
/// ```
/// # extern crate tower;
/// # extern crate tower_in_flight_limit;
/// # extern crate tower_buffer;
/// # extern crate tower_rate_limit;
/// # extern crate futures;
/// # extern crate void;
/// # use void::Void;
/// # use tower::Service;
/// # use tower::builder::ServiceBuilder;
/// # use tower_in_flight_limit::InFlightLimitLayer;
/// # use tower_buffer::BufferLayer;
/// # use tower_rate_limit::RateLimitLayer;
/// # use std::time::Duration;
/// # use futures::{Poll, future::{self, FutureResult}};
/// # #[derive(Debug)]
/// # struct MyService;
/// # impl Service<()> for MyService {
/// #    type Response = ();
/// #    type Error = Void;
/// #    type Future = FutureResult<Self::Response, Self::Error>;
/// #    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
/// #        Ok(().into())
/// #    }
/// #    fn call(&mut self, _: ()) -> Self::Future {
/// #        future::ok(())
/// #    }
/// # }
/// ServiceBuilder::new()
///     .chain(BufferLayer::new(5))
///     .chain(InFlightLimitLayer::new(5))
///     .chain(RateLimitLayer::new(5, Duration::from_secs(1)))
///     .build_svc(MyService);
/// ```
#[derive(Debug)]
pub struct ServiceBuilder<L, S, Request> {
    layer: L,
    _pd: PhantomData<(S, Request)>,
}

impl<S, Request> ServiceBuilder<Identity, S, Request> {
    /// Create a new `ServiceBuilder` from a `MakeService`.
    pub fn new() -> Self {
        ServiceBuilder {
            layer: Identity::new(),
            _pd: PhantomData,
        }
    }
}

impl<L, S, Request> ServiceBuilder<L, S, Request> {
    /// Chain a layer `T` to the `ServiceBuilder`.
    pub fn chain<T>(self, layer: T) -> ServiceBuilder<Chain<L, T>, S, Request>
    where
        L: Layer<S, Request>,
        T: Layer<L::Service, Request>,
    {
        ServiceBuilder {
            layer: self.layer.chain(layer),
            _pd: PhantomData,
        }
    }

    /// Create a `ServiceBuilderMaker` from the composed middleware and transport.
    pub fn build_maker<M, Target>(self, maker: M) -> ServiceBuilderMaker<M, L, Request>
    where
        M: MakeService<Target, Request, Service = S, Response = S::Response, Error = S::Error>,
        S: Service<Request>,
    {
        ServiceBuilderMaker::new(maker, self.layer)
    }

    /// Wrap the service `S` with the layers.
    pub fn build_svc(self, service: S) -> Result<L::Service, L::LayerError>
    where
        L: Layer<S, Request>,
        S: Service<Request>,
    {
        self.layer.layer(service)
    }
}

/// A no-op middleware.
///
/// When wrapping a `Service`, the `Identity` layer returns the provided
/// service without modifying it.
#[derive(Debug, Default, Clone)]
pub struct Identity {
    _p: (),
}

impl Identity {
    /// Create a new `Identity` value
    pub fn new() -> Identity {
        Identity { _p: () }
    }
}

/// Decorates a `Service`, transforming either the request or the response.
impl<S, Request> Layer<S, Request> for Identity
where
    S: Service<Request>,
    S::Error: Into<Error>,
{
    type Response = S::Response;
    type Error = S::Error;
    type LayerError = Never;
    type Service = S;

    fn layer(&self, inner: S) -> Result<Self::Service, Self::LayerError> {
        Ok(inner)
    }
}
