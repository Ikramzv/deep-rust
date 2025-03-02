use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    shutdown: Option<oneshot::Sender<String>>,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "CRATE_NAME=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (shutdown, rx) = oneshot::channel::<String>();

    let app_state = Arc::new(Mutex::new(AppState {
        shutdown: Some(shutdown),
    }));

    let app = Router::new()
        .route("/", get(index))
        .route("/shutdown", post(on_shutdown))
        .with_state(app_state);

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("Starting server...");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            // When this future returns, the server will shutdown

            let _ = rx.await;

            tracing::debug!("Shutting down the server...")
        })
        .await
        .unwrap();
}

async fn index() -> &'static str {
    "Hello, wrold"
}

async fn on_shutdown(State(state): State<Arc<Mutex<AppState>>>) {
    if let Some(shutdown) = state.lock().unwrap().shutdown.take() {
        let _ = shutdown.send("shutdown".to_string());
    }
}
