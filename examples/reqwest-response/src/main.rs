use std::{convert::Infallible, time::Duration};

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::Response,
    response::IntoResponse,
    routing::get,
    Router,
};
use reqwest::{header::CONTENT_TYPE, Client, StatusCode};
use tokio::net::TcpListener;
use tokio_stream::StreamExt;
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from(format!("debug, error")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    let client = Client::new();

    Router::new()
        .route("/", get(reqwest_response))
        .route("/stream", get(stream))
        .layer(TraceLayer::new_for_http().on_body_chunk(
            |chunk: &Bytes, _latency: Duration, _span: &Span| {
                tracing::info!("body chunk: {:?}", chunk);
            },
        ))
        .with_state(client)
}

async fn reqwest_response(State(client): State<Client>) -> impl IntoResponse {
    let response = match client.get("http://localhost:3000/stream").send().await {
        Ok(res) => res,
        Err(e) => {
            tracing::error!("Error sending reqwest request: {}", e);
            return (StatusCode::BAD_REQUEST, "Internal Server Error").into_response();
        }
    };

    let mut response_builder = Response::builder().status(response.status());

    *response_builder.headers_mut().unwrap() = response.headers().clone();

    tracing::info!("headers {:?}", response_builder.headers_ref().unwrap());

    response_builder
        .body(Body::from_stream(response.bytes_stream()))
        .unwrap()
}

async fn stream() -> impl IntoResponse {
    let stream = tokio_stream::iter(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
        .throttle(Duration::from_millis(500))
        .map(|x| x.to_string())
        .map(Ok::<_, Infallible>);

    let mut response = Response::builder();

    response
        .headers_mut()
        .unwrap()
        .insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());

    response.body(Body::from_stream(stream)).unwrap()
}
