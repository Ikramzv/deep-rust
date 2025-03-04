// tower::Service

// is roughly equivalent to an async function that takes a Request and returns a Response

// async fn a(Request) -> Result<Response, E> for some <Request, E>
// async fn b(Request) -> Result<Response, E> for some <Request, E>

// async fn c(req: Request) -> Result<Response, E> {
//     (a.layer(b))(req).await
// }

// ------------------------------------------------------------

use std::{future::Future, pin::Pin};
use tower::{Layer, Service};

#[derive(Debug, Clone)]
pub struct TestService<T>(pub T);

impl<T, Req> Service<Req> for TestService<T>
where
    T: Service<Req>,
    T::Response: 'static,
    T::Future: 'static,
    T::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let fut = self.0.call(req);

        Box::pin(async move {
            tracing::debug!("TestService called");
            fut.await
        })
    }
}

impl<S> Layer<S> for TestService<S> {
    type Service = TestService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TestService(inner)
    }
}
