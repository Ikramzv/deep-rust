use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::Arc;

pub struct AuthToken(pub String);

impl<S> FromRequestParts<S> for AuthToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .ok_or((StatusCode::UNAUTHORIZED, "No token provided".to_string()))?;

        let token = token
            .to_str()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let token = token
            .split_whitespace()
            .nth(1)
            .ok_or((StatusCode::BAD_REQUEST, "No token provided".to_string()))?
            .to_string();

        parts
            .extensions
            .insert(Arc::new(get_user_by_token(token.clone())));

        Ok(AuthToken(token))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct User {
    name: String,
    email: String,
    token: String,
}

fn get_user_by_token(token: String) -> User {
    User {
        name: "John Doe".to_string(),
        email: "john.doe@example.com".to_string(),
        token,
    }
}

impl IntoResponse for User {
    fn into_response(self) -> Response {
        let body = serde_json::to_string(&self).unwrap();
        Response::builder()
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap()
    }
}
