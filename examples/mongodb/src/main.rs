use std::sync::{Arc, LazyLock};

use axum_extra::{headers::ContentType, TypedHeader};
use serde_json::json;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use mongodb::{
    bson::{doc, DateTime},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_name = std::env::var("MONGO_DB_NAME").unwrap_or("test".to_owned());

    Config {
        database_url,
        db_name: db_name.into(),
    }
});

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug, info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    debug!("DATABASE_URL: {}", CONFIG.database_url);

    let client = Client::with_uri_str(&CONFIG.database_url).await.unwrap();

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app(client)).await.unwrap()
}

fn app(client: Client) -> Router {
    let collection: Users = Arc::new(client.database(&CONFIG.db_name).collection("users"));

    let cloned = Arc::clone(&collection);

    tokio::spawn(async move {
        setup_collection(cloned).await;
        debug!("setup collection finished");
    });

    Router::new()
        .route("/create", post(create_user))
        .route("/get/{name}", get(get_user))
        .route("/update/{name}", patch(update_user))
        .route("/delete/{name}", delete(delete_user))
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::clone(&collection))
}

type Users = Arc<Collection<User>>;

async fn create_user(
    State(users): State<Users>,
    Json(new_user): Json<NewUserDto>,
) -> impl IntoResponse {
    let user = User {
        display_name: new_user.display_name,
        email: new_user.email,
        created_at: DateTime::now(),
        updated_at: DateTime::now(),
    };

    let result = users.insert_one(user.clone()).await.unwrap();

    debug!(
        "inserted user with id: {}\nResult:{:?}",
        result.inserted_id, result
    );

    Json(user)
}

async fn get_user(State(users): State<Users>, Path(name): Path<String>) -> Response {
    let Ok(Some(user)) = users
        .find_one(doc! {
            "displayName": name
        })
        .await
    else {
        return (
            StatusCode::NOT_FOUND,
            TypedHeader(ContentType::json()),
            json!({
                "message": "User not found"
            })
            .to_string(),
        )
            .into_response();
    };

    (StatusCode::OK, Json(user)).into_response()
}

async fn update_user(
    State(users): State<Users>,
    Path(name): Path<String>,
    Json(body): Json<UpdateUserDto>,
) -> Result<Response, Response> {
    if body.display_name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            TypedHeader(ContentType::json()),
            json!({ "message": "Display name is required" }).to_string(),
        )
            .into_response());
    }

    let user = users
        .find_one_and_update(
            doc! { "displayName": name },
            doc! {
                    "$set": doc! {
                        "displayName": body.display_name,
                        "updatedAt": DateTime::now()
                    },

            },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                TypedHeader(ContentType::json()),
                json!({ "message": err.to_string() }).to_string(),
            )
                .into_response()
        })?;

    if user.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            TypedHeader(ContentType::json()),
            json!({ "message": "User not found" }).to_string(),
        )
            .into_response());
    }

    Ok((StatusCode::OK, Json(user)).into_response())
}

async fn delete_user(
    State(users): State<Users>,
    Path(name): Path<String>,
) -> Result<Response, Response> {
    let result = users
        .find_one_and_delete(doc! {
            "displayName": name
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                TypedHeader(ContentType::json()),
                json!({ "message": e.to_string() }).to_string(),
            )
                .into_response()
        })?;

    if result.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            TypedHeader(ContentType::json()),
            json!({ "message": "User not found" }).to_string(),
        )
            .into_response());
    }

    Ok((
        StatusCode::OK,
        TypedHeader(ContentType::json()),
        json!({ "message": "User deleted" }).to_string(),
    )
        .into_response())
}

async fn setup_collection(collection: Users) {
    use mongodb::{bson::doc, options::IndexOptions, IndexModel};

    let options = IndexOptions::builder().unique(true).build();

    let email_index = IndexModel::builder()
        .keys(doc! { "email": 1 })
        .options(options.clone())
        .build();

    let display_name_index = IndexModel::builder()
        .keys(doc! { "displayName": 1 })
        .options(options)
        .build();

    let indexes = vec![email_index, display_name_index];

    let result = collection.create_indexes(indexes).await.unwrap();

    debug!("create index result {:?}", result);
}

#[derive(Debug, Clone)]
struct Config {
    database_url: String,
    db_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    #[serde(rename = "displayName")]
    display_name: String,
    email: String,
    created_at: DateTime,
    updated_at: DateTime,
}

#[derive(Debug, Deserialize)]
struct NewUserDto {
    #[serde(rename = "displayName")]
    display_name: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct UpdateUserDto {
    #[serde(rename = "displayName")]
    display_name: String,
}
