use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use axum::{
    body::Bytes,
    error_handling::HandleErrorLayer,
    extract::{DefaultBodyLimit, Path, State},
    handler::Handler,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
    BoxError, Json, Router,
};
use tokio::{
    io::{AsyncReadExt, BufReader},
    net::TcpListener,
};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, limit::RequestBodyLimitLayer, trace::TraceLayer,
    validate_request::ValidateRequestHeaderLayer,
};
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

    let shared_state = SharedState::default();

    let app = Router::new()
        .route(
            "/{key}",
            get(kv_get.layer(CompressionLayer::new())).post_service(
                kv_set
                    .layer((
                        DefaultBodyLimit::disable(),
                        RequestBodyLimitLayer::new(1024 * 1024 * 5),
                    ))
                    .with_state(Arc::clone(&shared_state)),
            ),
        )
        .route("/keys", get(list_keys))
        .nest("/admin", admin_routes())
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(Arc::clone(&shared_state));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("Server listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

type SharedState = Arc<RwLock<AppState>>;

#[derive(Debug, Default)]
struct AppState {
    db: HashMap<String, Bytes>,
}

async fn kv_get(
    Path(key): Path<String>,
    State(state): State<SharedState>,
) -> Result<Bytes, StatusCode> {
    let db = &state.read().unwrap().db;

    if let Some(value) = db.get(&key) {
        return Ok(value.clone());
    }

    Err(StatusCode::NOT_FOUND)
}

async fn kv_set(
    Path(key): Path<String>,
    State(state): State<SharedState>,
    bytes: Bytes, // Gives the raw request body
) -> Result<StatusCode, ()> {
    state.write().unwrap().db.insert(key, bytes);

    Ok(StatusCode::CREATED)
}

async fn list_keys(State(state): State<SharedState>) -> Result<Response, ()> {
    let db = &state.read().unwrap().db;

    let keys = db
        .keys()
        .map(|key| key.to_string())
        .collect::<Vec<String>>();

    Ok((StatusCode::OK, Json(keys)).into_response())
}

fn admin_routes() -> Router<SharedState> {
    async fn delete_all_keys(State(state): State<SharedState>) {
        state.write().unwrap().db.clear();
    }

    async fn remove_key(Path(key): Path<String>, State(state): State<SharedState>) {
        state.write().unwrap().db.remove(&key);
    }

    Router::new()
        .route("/keys", delete(delete_all_keys))
        .route("/key/{key}", delete(remove_key))
        .layer(ValidateRequestHeaderLayer::bearer("secret-token"))
}

async fn handle_error(err: BoxError) -> impl IntoResponse {
    if err.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, "Request timed out");
    }

    if err.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Service overloaded, try again later",
        );
    }

    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
}
