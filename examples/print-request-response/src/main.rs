use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use http_body_util::BodyExt;
use serde::Serialize;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(index))
        .layer(axum::middleware::from_fn(logger));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Serialize)]
struct IndexResponse {
    message: String,
}

async fn index() -> Json<IndexResponse> {
    Json(IndexResponse {
        message: "Hello, world!".to_string(),
    })
}

async fn logger(req: Request, next: Next) -> Result<Response, (StatusCode, String)> {
    let (parts, body) = req.into_parts();
    let bytes = print_body("request", body).await?;
    let req = Request::from_parts(parts, Body::from(bytes));

    let res = next.run(req).await;
    let (parts, body) = res.into_parts();
    let bytes = print_body("response", body).await?;

    let res = Response::from_parts(parts, Body::from(bytes));

    Ok(res)
}

async fn print_body(direction: &str, body: Body) -> Result<Bytes, (StatusCode, String)> {
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("Failed to collect body: {}", err),
            ))
        }
    };

    if let Ok(body) = std::str::from_utf8(&bytes) {
        tracing::info!("{} body: {}", direction, body);
    }

    Ok(bytes)
}

// async fn collect_bytes(body: Body) -> Bytes {
//     let mut bytes: Vec<Bytes> = Vec::new();

//     let mut data_stream = body.into_data_stream();
//     let stream = Pin::new(&mut data_stream);

//     poll_fn(|cx| match stream.poll_frame(cx) {
//         Poll::Pending => Poll::Pending,
//         Poll::Ready(Some(Ok(frame))) => {
//             let data = frame.into_data().unwrap();
//             bytes.push(data);
//             Poll::Pending
//         }
//         Poll::Ready(None) => Poll::Ready(Bytes::new()),
//         Poll::Ready(Some(Err(err))) => {
//             tracing::error!("Error reading body: {}", err);
//             Poll::Ready(Bytes::new())
//         }
//     })
//     .await
// }
