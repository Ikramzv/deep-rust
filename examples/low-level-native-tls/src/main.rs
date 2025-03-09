use std::path::PathBuf;

use axum::{routing::get, Router};
use futures_util::pin_mut;
use hyper::{body::Incoming, Request};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio_native_tls::{
    native_tls::{Identity, Protocol, TlsAcceptor as NativeTlsAcceptor},
    TlsAcceptor,
};
use tower_service::Service;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_PKG_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let tls_acceptor = native_tls_acceptor(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("certs")
            .join("key.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("certs")
            .join("cert.pem"),
    );

    let tls_acceptor = TlsAcceptor::from(tls_acceptor);
    let tcp_listener = TcpListener::bind("localhost:3000").await.unwrap();
    tracing::info!("Listening on {}", tcp_listener.local_addr().unwrap());

    let app = Router::new().route("/", get(handler));

    pin_mut!(tcp_listener);

    loop {
        let tower_service = app.clone();
        let tls_acceptor = tls_acceptor.clone();

        let (conn, addr) = tcp_listener.accept().await.unwrap();

        tokio::spawn(async move {
            let Ok(stream) = tls_acceptor.accept(conn).await else {
                tracing::error!("error during tls handshake: {}", addr);
                return;
            };

            let stream = TokioIo::new(stream);

            let hyper_service = hyper::service::service_fn(|request: Request<Incoming>| {
                tower_service.clone().call(request)
            });

            let result = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(stream, hyper_service)
                .await;

            if let Err(err) = result {
                tracing::warn!("error serving connection: {}", err);
            }
        });
    }
}

async fn handler() -> &'static str {
    "App is running"
}

fn native_tls_acceptor(key_file: PathBuf, cert_file: PathBuf) -> NativeTlsAcceptor {
    let key_pem = std::fs::read_to_string(key_file).unwrap();
    let cert_pem = std::fs::read_to_string(cert_file).unwrap();

    let id = Identity::from_pkcs8(key_pem.as_bytes(), cert_pem.as_bytes()).unwrap();

    NativeTlsAcceptor::builder(id)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
        .unwrap()
}
