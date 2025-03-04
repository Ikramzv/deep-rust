use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use axum::{
    extract::{rejection::JsonRejection, FromRequest, MatchedPath, Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "CRATE_NAME=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/users", post(create_user))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request| {
                    let method = req.method();
                    let uri = req.uri();

                    let matched_path = req.extensions().get::<MatchedPath>().map(|m| m.as_str());

                    tracing::debug_span!("request", %method, %uri, matched_path)
                })
                .on_failure(()),
        )
        .with_state(AppState::default());

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Default, Clone)]
struct AppState {
    next_id: Arc<AtomicU64>,
    users: Arc<Mutex<HashMap<u64, User>>>,
}

#[derive(Deserialize)]
struct UserParams {
    name: String,
}

#[derive(Debug, Serialize, Clone)]
struct User {
    id: u64,
    name: String,
    created_at: std::time::SystemTime,
}

async fn create_user(
    State(state): State<AppState>,
    AppJson(params): AppJson<UserParams>,
) -> Result<AppJson<User>, AppError> {
    let id = state.next_id.fetch_add(1, Ordering::SeqCst);
    let created_at = std::time::SystemTime::now();

    let user = User {
        created_at,
        id,
        name: params.name,
    };

    state.users.lock().unwrap().insert(id, user.clone());

    Ok(AppJson(user))
}

// derive FromRequest macro implements the FromRequest trait for AppJson
#[derive(FromRequest)]
// from_request macro configures how the FromRequest trait is implemented
// - via(axum::Json) specifies that it uses standard axum:;Json extractor as its base
// - rejection(AppError) specifies that it should use AppError as its rejection type
// AppJson should implement From<JsonRejection> to convert JsonRejection to AppError
#[from_request(via(axum::Json), rejection(AppError))]
struct AppJson<T>(T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

enum AppError {
    JsonRejection(JsonRejection),
    TimeError(std::time::SystemTimeError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        struct Error {
            message: String,
        }

        let (status, message) = match self {
            AppError::JsonRejection(rejection) => (rejection.status(), rejection.body_text()),
            AppError::TimeError(err) => {
                tracing::error!(%err, "error from system time");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, axum::Json(Error { message })).into_response()
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}
