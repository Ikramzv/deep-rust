use axum::{
    extract::{DefaultBodyLimit, Multipart},
    response::Html,
    routing::get,
    Router,
};

use tokio::net::TcpListener;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static MAX_BODY_SIZE: usize = 1024 * 1024 * 250; // 250MB

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
        .route("/", get(index).post(accept_multipart))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();

    println!("Hello, world!");
}

async fn index() -> Html<&'static str> {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>Multipart Form</title>
            </head>
            <body>
                <h1>Multipart Form</h1>
                <form method="post" action="/" enctype="multipart/form-data">
                    <input type="file" name="file" />
                    <input type="submit" value="Upload files" />
                </form>
            </body>
        </html
    "#,
    )
}

async fn accept_multipart(mut multipart: Multipart) {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let file_name = field.file_name().unwrap().to_string();
        let content_type = field.content_type().unwrap().to_string();
        let bytes = field.bytes().await.unwrap();

        let chunk_size = 1024 * 1024;

        bytes.chunks(chunk_size).enumerate().for_each(|(i, chunk)| {
            let start = i * chunk_size;
            let end = start + chunk.len();
            tracing::debug!(
                "name={}, file_name={}, content_type={}, start={}, end={}",
                name,
                file_name,
                content_type,
                start,
                end
            );
        });
    }
}
