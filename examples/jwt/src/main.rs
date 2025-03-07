use std::{fmt::Display, sync::LazyLock};

use axum::{
    extract::FromRequestParts,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, RequestPartsExt, Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization, ContentType},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static KEYS: LazyLock<Keys> = LazyLock::new(|| {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    Keys::new(secret.as_bytes())
});

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug,error", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/protected", get(protected))
        .route("/authorize", post(authorize));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("Server is listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn protected(claims: Claims) -> Result<Response, AuthError> {
    Ok((
        StatusCode::OK,
        TypedHeader(ContentType::json()),
        json!({
            "message": "Welcome to protected route",
            "data": claims
        })
        .to_string(),
    )
        .into_response())
}

async fn authorize(Json(data): Json<AuthPayload>) -> Result<Json<AuthBody>, AuthError> {
    if data.client_id.is_empty() || data.client_secret.is_empty() {
        return Err(AuthError::MissingCredentials);
    }

    let now = std::time::SystemTime::now();

    let exp = (now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 10) as usize;

    let claims = Claims {
        sub: "ikramzulfugar@gmail.com".to_owned(),
        company: "ikramzulfugar.co".to_owned(),
        exp,
    };

    let token = encode(&Header::default(), &claims, &KEYS.encoding).unwrap();

    Ok(Json(AuthBody::new(token)))
}

impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Claims Email: {}\nCompany: {}", self.sub, self.company)
    }
}

impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Claims, Self::Rejection> {
        let TypedHeader(Authorization(token)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .unwrap();

        let token = token.token();

        let mut validation = Validation::default();

        validation.leeway = 0;

        let data = decode::<Claims>(token, &KEYS.decoding, &validation)
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(data.claims)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let (status_code, message) = match self {
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::MissingCredentials => (StatusCode::UNAUTHORIZED, "Missing credentials"),
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
        };

        tracing::error!("AuthError: {}", message);

        (status_code, message).into_response()
    }
}

impl AuthBody {
    fn new(token: String) -> Self {
        Self {
            access_token: token,
            token_type: "Bearer".to_string(),
        }
    }
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    company: String,
    exp: usize,
}

#[derive(Debug, Serialize)]
struct AuthBody {
    access_token: String,
    token_type: String,
}

#[derive(Debug, Deserialize)]
struct AuthPayload {
    client_id: String,
    client_secret: String,
}

#[derive(Debug)]
enum AuthError {
    WrongCredentials,
    InvalidToken,
    MissingCredentials,
    TokenCreation,
}
