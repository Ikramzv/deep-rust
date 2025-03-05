mod config;
mod models;
mod schema;

use std::sync::Arc;

use axum::{
    extract::{FromRef, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use diesel::{query_dsl::methods::SelectDsl, Insertable, SelectableHelper};
use diesel_async::{
    pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection, RunQueryDsl,
};
use models::NewPost;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

type Pool = bb8::Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_default())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_config = config::Config::new();

    let pool_config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(
        app_config.database_url,
    );

    let pool = bb8::Pool::builder().build(pool_config).await.unwrap();

    let app = Router::new()
        .route("/posts", get(get_posts))
        .route("/posts", post(create_post))
        .with_state(Arc::new(pool));

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

#[axum::debug_handler]
async fn get_posts(State(pool): State<Arc<Pool>>) -> Json<Vec<models::Post>> {
    let mut conn = pool.get().await.unwrap();

    let posts = schema::posts::table
        .select(models::Post::as_select())
        .load(&mut conn)
        .await
        .unwrap();

    Json(posts)
}

#[axum::debug_handler]
async fn create_post(
    State(pool): State<Arc<Pool>>,
    Json(new_post): Json<models::NewPost>,
) -> impl IntoResponse {
    let mut conn = pool.get().await.unwrap();

    let new_post = diesel::insert_into(schema::posts::table)
        .values(new_post)
        .returning(models::Post::as_returning())
        .get_result(&mut conn)
        .await
        .unwrap();

    Json(new_post)
}
