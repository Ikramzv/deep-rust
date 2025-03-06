use std::net::SocketAddr;

use axum::{
    body::Body,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use hyper::{body::Incoming, server::conn::http1, upgrade::Upgraded, Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};
use tower::{Service, ServiceExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=trace,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .init();

    let router_svc = Router::new().route("/", get(|| async { "Hello, World !" }));

    let tower_service = tower::service_fn(move |req: Request<_>| {
        let router_svc = router_svc.clone();
        let req = req.map(Body::new);
        async move {
            // Handle CONNECT requests for tunneling
            if req.method() == Method::CONNECT {
                proxy(req).await.map_err(|err| err.to_string())
            } else {
                // Handle other requests by routing them to the router
                router_svc.oneshot(req).await.map_err(|err| err.to_string())
            }
        }
    });

    let hyper_service =
        hyper::service::service_fn(move |req: Request<Incoming>| tower_service.clone().call(req));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let hyper_service = hyper_service.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .serve_connection(io, hyper_service)
                .with_upgrades()
                .await
            {
                tracing::error!("Error serving connection: {:?}", err);
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn proxy(req: Request<Body>) -> Result<Response, hyper::Error> {
    tracing::trace!(?req);

    if let Some(host_addr) = req.uri().authority().map(|auth| auth.to_string()) {
        tokio::task::spawn(async move {
            // Upgrade the connection and create a raw TCP connection
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    // Handle the upgraded connection
                    if let Err(e) = tunnel(upgraded, host_addr).await {
                        tracing::warn!("server io error {:?}", e);
                    }
                }
                Err(err) => {
                    tracing::warn!("upgrade error {:?}", err);
                }
            }
        });

        Ok(Response::new(Body::empty()))
    } else {
        tracing::warn!("CONNECT host is not socker addr: {:?}", req.uri());

        Ok((
            StatusCode::BAD_REQUEST,
            "CONNECT must be to a socket address",
        )
            .into_response())
    }
}

async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);

    // Write/read data bidirectionally between the client and the server
    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    tracing::debug!(
        "client wrote {} bytes and received {} bytes",
        from_client,
        from_server
    );

    Ok(())
}
