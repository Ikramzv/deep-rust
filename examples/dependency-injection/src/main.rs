use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
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

    let user_repo = InMemoryUserRepo::default();

    let app = Router::new()
        .route("/users", get(get_users))
        .route("/users", post(create_user))
        .with_state(Arc::new(Mutex::new(AppState {
            user_repo: Arc::new(user_repo),
        })));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();

    println!("Hello, world!");
}

#[derive(Clone)]
struct AppState {
    user_repo: Arc<dyn UserRepo>,
}

async fn get_users(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let users = state.lock().unwrap().user_repo.get_users();
    Json(users)
}

#[axum::debug_handler]
async fn create_user(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(user): Json<User>,
) -> impl IntoResponse {
    let user = state.lock().unwrap().user_repo.create_user(user);
    Json(user)
}

trait UserRepo: Send + Sync {
    fn get_users(&self) -> Vec<User>;
    fn create_user(&self, user: User) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
}

#[derive(Default, Debug)]
struct InMemoryUserRepo {
    map: Arc<Mutex<HashMap<String, User>>>,
}

impl UserRepo for InMemoryUserRepo {
    fn get_users(&self) -> Vec<User> {
        self.map.lock().unwrap().values().cloned().collect()
    }

    fn create_user(&self, user: User) -> bool {
        self.map.lock().unwrap().insert(user.id.clone(), user);
        true
    }
}
