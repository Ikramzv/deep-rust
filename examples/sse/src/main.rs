use std::{convert::Infallible, path::PathBuf, time::Duration};

use axum::{
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::get,
    Router,
};
use axum_extra::TypedHeader;
use futures::{stream, Stream};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_stream::StreamExt;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from("debug, error"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = app();

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    println!("assets_dir: {:?}", assets_dir);
    let serve_dir = ServeDir::new(assets_dir).append_index_html_on_directories(true);
    Router::new()
        .fallback_service(serve_dir)
        .route("/sse", get(sse_handler))
        .layer(TraceLayer::new_for_http())
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::debug!("SSE connection from {}", user_agent.to_string());

    let (tx, rx) = mpsc::channel::<String>(32);

    let mut sys = sysinfo::System::new_all();

    tokio::spawn(async move {
        loop {
            sys.refresh_all();
            sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
            sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::everything());
            let cpu_usage = sys.global_cpu_usage();
            let used_memory = sys.used_memory() as f64;
            let total_memory = sys.total_memory() as f64;
            let memory_usage = used_memory / total_memory;
            let message = format!(
                "CPU: {}%, Memory: {}%",
                cpu_usage,
                (memory_usage * 100.0) as u64 as f64 / 100.0
            );

            if let Err(_) = tx.send(message).await {
                tracing::error!("Failed to send message to channel");
                break;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    let stream = stream::unfold(rx, |mut rx| async {
        if let Some(message) = rx.recv().await {
            return Some((Event::default().data(message), rx));
        }

        Some((
            Event::default()
                .event("error")
                .data("No message from server"),
            rx,
        ))
    })
    .map(Ok);

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_millis(500))
            .text("Keep alive message"),
    )
}

#[cfg(test)]
mod tests {
    use eventsource_stream::Eventsource;
    use reqwest_eventsource::RequestBuilderExt;

    use super::*;

    async fn spawn_app(host: impl Into<String>) -> String {
        let host = host.into();
        let port = 3000;
        let listener = TcpListener::bind(format!("{}:{}", &host, port))
            .await
            .unwrap();
        let app = app();

        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        format!("http://{}:{}", host, port)
    }

    #[tokio::test]
    async fn test_sse_handler() {
        let url = spawn_app("localhost").await;
        let mut event_stream = reqwest::Client::new()
            .get(format!("{}/sse", url))
            .header("User-Agent", "sse-integration-tests")
            .send()
            .await
            .unwrap()
            .bytes_stream()
            .eventsource()
            .take(3);

        let mut messages = Vec::<String>::new();

        while let Some(event) = event_stream.next().await {
            match event {
                Ok(event) => {
                    messages.push(event.data);
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    break;
                }
            }
        }

        println!("Messages: {:#?}", messages);

        assert_eq!(messages.len(), 3);
        assert!(messages[0].contains("CPU"));
        assert!(messages[1].contains("Memory"));
        assert!(messages[2].contains("CPU"));
    }
}
