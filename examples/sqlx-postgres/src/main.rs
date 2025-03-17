use std::{
    sync::{Arc, LazyLock},
    time::Duration,
};

use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    routing::get,
    Router,
};
use sqlx::{postgres::PgPoolOptions, PgPool, Pool};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::new());

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug, error, info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(2))
        .connect(&CONFIG.db_url)
        .await
        .inspect_err(|e| eprintln!("{e}"))
        .expect("Failed to connect to DB");

    let app = Router::new()
        .route("/", get(index))
        .with_state(Arc::new(pool));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn index(State(pool): State<Arc<PgPool>>) -> Result<String, StatusCode> {
    let result = sqlx::query_scalar("SELECT 'Hello, World!'")
        .fetch_one(&*pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(result)
}

#[derive(Debug)]
struct Config {
    db_url: String,
}

impl Config {
    fn new() -> Self {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        Self { db_url }
    }
}
