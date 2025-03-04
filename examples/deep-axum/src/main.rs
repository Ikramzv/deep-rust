mod extractors;
mod services;

use std::sync::{Arc, Mutex};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
    routing::{get, post},
    Extension, Router,
};

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower::ServiceBuilder;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use extractors::{AuthToken, User};

struct AppState {
    shutdown: Option<oneshot::Sender<String>>,
}

struct AppState2;

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
        .with_state(app_state)
        .with_state(Arc::new(AppState2))
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(
                    |mut req: Request, next: Next| async move {
                        tracing::info!("Middleware 1");

                        req.extensions_mut().insert("hello world");
                        req.extensions_mut().insert("hello world 2");

                        let response = next.run(req).await;

                        response
                    },
                ))
                .layer(axum::middleware::from_fn(
                    |req: Request, next: Next| async move {
                        tracing::info!("Middleware 2");

                        tracing::info!("{:?}", req.extensions().get::<&'static str>());

                        let response = next.run(req).await;

                        response
                    },
                )),
        );

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

async fn index(
    AuthToken(token): AuthToken,
    Extension(user): Extension<Arc<User>>,
) -> impl IntoResponse {
    tracing::info!("Token: {}", token);

    (StatusCode::OK, (*user).clone().into_response())
}

async fn on_shutdown(State(state): State<Arc<Mutex<AppState>>>) {
    if let Some(shutdown) = state.lock().unwrap().shutdown.take() {
        let _ = shutdown.send("shutdown".to_string());
    }
}
