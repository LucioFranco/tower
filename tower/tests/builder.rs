extern crate futures;
extern crate tokio;
extern crate tower;
extern crate tower_buffer;
extern crate tower_in_flight_limit;
extern crate tower_layer;
extern crate tower_rate_limit;
extern crate tower_reconnect;
extern crate tower_retry;
extern crate tower_service;
extern crate void;

use futures::future::{self, FutureResult};
use futures::prelude::*;
use std::time::Duration;
use tower::builder::ServiceBuilder;
use tower_buffer::BufferLayer;
use tower_in_flight_limit::InFlightLimitLayer;
use tower_rate_limit::RateLimitLayer;
use tower_reconnect::Reconnect;
use tower_retry::{Policy, RetryLayer};
use tower_service::*;
use void::Void;

#[test]
fn builder_make_service() {
    tokio::run(future::lazy(|| {
        let maker = ServiceBuilder::new()
            .chain(BufferLayer::new(5))
            .chain(InFlightLimitLayer::new(5))
            .chain(RateLimitLayer::new(5, Duration::from_secs(1)))
            .build_maker(MockMaker);

        let mut client = Reconnect::new(maker, ());

        client.poll_ready().unwrap();
        client
            .call(Request)
            .map(|_| ())
            .map_err(|_| panic!("this is bad"))
    }));
}

#[test]
fn builder_service() {
    tokio::run(future::lazy(|| {
        let mut client = ServiceBuilder::new()
            .chain(BufferLayer::new(5))
            .chain(InFlightLimitLayer::new(5))
            .chain(RateLimitLayer::new(5, Duration::from_secs(1)))
            .build_svc(MockSvc)
            .unwrap();

        client.poll_ready().unwrap();
        client
            .call(Request)
            .map(|_| ())
            .map_err(|_| panic!("this is bad"))
    }));
}

#[test]
fn builder_make_service_retry() {
    tokio::run(future::lazy(|| {
        let policy = MockPolicy;

        let maker = ServiceBuilder::new()
            .chain(BufferLayer::new(5))
            .chain(RetryLayer::new(policy))
            .chain(InFlightLimitLayer::new(5))
            .chain(RateLimitLayer::new(5, Duration::from_secs(1)))
            .build_maker(MockMaker);

        let mut client = Reconnect::new(maker, ());

        client.poll_ready().unwrap();
        client
            .call(Request)
            .map(|_| ())
            .map_err(|_| panic!("this is bad"))
    }));
}

#[derive(Debug)]
struct MockMaker;
impl Service<()> for MockMaker {
    type Response = MockSvc;
    type Error = Void;
    type Future = FutureResult<Self::Response, Self::Error>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(().into())
    }

    fn call(&mut self, _: ()) -> Self::Future {
        future::ok(MockSvc)
    }
}

#[derive(Debug, Clone)]
struct Request;
#[derive(Debug, Clone)]
struct Response;
#[derive(Debug)]
struct MockSvc;
impl Service<Request> for MockSvc {
    type Response = Response;
    type Error = Void;
    type Future = FutureResult<Self::Response, Self::Error>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(().into())
    }

    fn call(&mut self, _: Request) -> Self::Future {
        future::ok(Response)
    }
}

#[derive(Debug, Clone)]
struct MockPolicy;

impl<E> Policy<Request, Response, E> for MockPolicy
where
    E: Into<Box<std::error::Error + Send + Sync + 'static>>,
{
    type Future = FutureResult<Self, ()>;

    fn retry(&self, _req: &Request, _result: Result<&Response, &E>) -> Option<Self::Future> {
        None
    }

    fn clone_request(&self, req: &Request) -> Option<Request> {
        Some(req.clone())
    }
}
