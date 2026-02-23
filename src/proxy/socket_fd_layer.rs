use axum::extract::Request;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Extension that stores the socket file descriptor
#[derive(Clone, Copy, Debug)]
pub struct SocketFd(pub i32);

/// Layer that extracts socket FD and adds it to request extensions
#[derive(Clone)]
#[allow(dead_code)]
pub struct SocketFdLayer;

impl<S> Layer<S> for SocketFdLayer {
    type Service = SocketFdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SocketFdService { inner }
    }
}

/// Service that extracts socket FD
#[derive(Clone)]
#[allow(dead_code)]
pub struct SocketFdService<S> {
    inner: S,
}

impl<S, B> Service<Request<B>> for SocketFdService<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        // Note: We can't actually get the socket FD here because Axum/Hyper
        // doesn't expose it at this layer. The TCP stream is owned by Hyper
        // and not accessible from the request.
        //
        // To get the FD, we'd need to:
        // 1. Use hyper directly with a custom Connector
        // 2. Use a lower-level server like tokio::net::TcpListener directly
        // 3. Use axum::serve with a custom MakeService that captures the stream
        //
        // For now, we'll need to use approach #3 with a custom connection handler
        
        self.inner.call(request)
    }
}
