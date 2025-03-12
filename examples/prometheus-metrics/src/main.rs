use std::{
    future::ready,
    time::{Duration, Instant},
};

use axum::{
    extract::{MatchedPath, Request},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Router,
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let (_main_server, _metrics_server) = tokio::join!(start_main_server(), start_metrics_server());
}

async fn start_main_server() {
    let app = main_app();

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    tracing::info!("Starting main server on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn start_metrics_server() {
    let app = metrics_app();

    let listener = TcpListener::bind("127.0.0.1:3001").await.unwrap();

    tracing::info!(
        "Starting metrics server on {}",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}

fn main_app() -> Router {
    Router::new().route("/fast", get(|| async {})).route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(2)).await;
        })
        .route_layer(middleware::from_fn(track_metrics)),
    )
}

fn metrics_app() -> Router {
    let recorder_handle = setup_metrics_recorder();
    Router::new().route("/metrics", get(move || ready(recorder_handle.render())))
}

fn setup_metrics_recorder() -> PrometheusHandle {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .install_recorder()
        .unwrap()
}

async fn track_metrics(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_string()
    } else {
        req.uri().path().to_string()
    };

    let method = req.method().to_string();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [("method", method), ("path", path), ("status", status)];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

    response
}
