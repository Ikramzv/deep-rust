use std::{convert::Infallible, env};

use anyhow::{anyhow, Context};
use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    RequestPartsExt, Router,
};
use axum_extra::{headers, typed_header::TypedHeaderRejectionReason, TypedHeader};
use http::{header, request::Parts, HeaderMap};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static COOKIE_NAME: &str = "SESSION";
static CSRF_TOKEN: &str = "csrf_token";

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug,error", env!("CARGO_PKG_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let store = MemoryStore::new();

    let oauth_client = oauth_client().unwrap();

    let app_state = AppState {
        store,
        oauth_client,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/auth/github", get(auth_github))
        .route("/auth/callback", get(auth_github_callback)) // redirect url
        .route("/protected", get(protected))
        .route("/logout", get(logout))
        .with_state(app_state);

    let listener = TcpListener::bind("localhost:3000")
        .await
        .context("failed to bind TcpListener")
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    store: MemoryStore,
    oauth_client: BasicClient,
}

impl FromRef<AppState> for MemoryStore {
    fn from_ref(state: &AppState) -> Self {
        state.store.clone()
    }
}

impl FromRef<AppState> for BasicClient {
    fn from_ref(state: &AppState) -> Self {
        state.oauth_client.clone()
    }
}

fn oauth_client() -> Result<BasicClient, AppError> {
    let client_id: String = env::var("CLIENT_ID").context("Missing CLIENT_ID")?;
    let client_secret = env::var("CLIENT_SECRET").context("Missing CLIENT_SECRET")?;
    let redirect_uri = env::var("REDIRECT_URL").context("Missing REDIRECT_URL")?;

    let auth_url = env::var("AUTH_URL").context("Missing AUTH_URL")?;

    let token_url = env::var("TOKEN_URL").context("Missing TOKEN_URL")?;

    let basic_client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(auth_url)?,
        Some(TokenUrl::new(token_url)?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_uri.to_string())?);

    Ok(basic_client)
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    avatar_url: String,
    bio: String,
    created_at: String,
    email: String,
    followers: usize,
    following: usize,
    html_url: String,
    name: String,
    location: String,
    url: String,
}

async fn index(user: Option<User>) -> impl IntoResponse {
    match user {
        Some(u) => Html(format!(
            r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <h1>Hello, {}!</h1>
                    <p>You're logged in!</p>
                    <p>Visit <a href="/protected">/protected</a> to access the protected route.</p>
                    <p>Or <a href="/logout">/logout</a> to log out.</p>
                </body>
            </html>
        "#,
            u.name
        )),
        None => Html(
            r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <h1>You're not logged in.</h1>
                    <p>Visit <a href="/auth/github">/auth/github</a> to do so.</p>
                </body>
            </html>
        "#
            .to_string(),
        ),
    }
}

// Store the CSRF token in the session, then redirect user to the auth url
async fn auth_github(
    State(client): State<BasicClient>,
    State(store): State<MemoryStore>,
) -> Result<impl IntoResponse, AppError> {
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user:email".to_string()))
        .add_extra_param("prompt", "consent")
        .url();

    let mut session = Session::new();

    session
        .insert(CSRF_TOKEN, &csrf_token)
        .context("failed to insert CSRF token into session")?;

    let cookie = store
        .store_session(session)
        .await
        .context("failed to store session")?
        .context("failed to retrieve session cookie")?;

    let cookie = format!("{COOKIE_NAME}={cookie}; Path=/; Secure; HttpOnly; SameSite=Lax");
    let mut headers = HeaderMap::new();

    headers.insert(
        header::SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to(auth_url.as_ref())))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthRequest {
    code: String,
    state: String,
}

async fn validate_csrf_token(
    query: &AuthRequest,
    cookies: &headers::Cookie,
    store: &MemoryStore,
) -> Result<(), AppError> {
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie")?
        .to_string();

    let session = match store
        .load_session(cookie)
        .await
        .context("Failed to load session")?
    {
        Some(session) => session,
        None => return Err(anyhow!("Session not found").into()),
    };

    let stored_csrf_token = session
        .get::<CsrfToken>(CSRF_TOKEN)
        .context("Failed to get CSRF token")?;

    store
        .destroy_session(session)
        .await
        .context("Failed to destroy session")?;

    // Verify CSRF token is the same as the one in the auth request
    if *stored_csrf_token.secret() != query.state {
        return Err(anyhow!("CSRF token mismatch").into());
    }

    Ok(())
}

async fn auth_github_callback(
    Query(query): Query<AuthRequest>,
    State(store): State<MemoryStore>,
    State(client): State<BasicClient>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    validate_csrf_token(&query, &cookies, &store).await?;

    let token = client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .context("Failed to exchange code for token")?;

    let client = reqwest::Client::new();

    let response = client
        .get("https://api.github.com/user")
        .bearer_auth(token.access_token().secret())
        .header("User-Agent", "axum-oauth-example")
        .send()
        .await
        .context("Failed to send request to get user data from GitHub")?;

    if response.status().is_client_error() || response.status().is_server_error() {
        let message = response
            .text()
            .await
            .unwrap_or("Something went wrong".to_string());
        return Err(anyhow!(message).into());
    }

    let user = response.json::<User>().await.unwrap();

    tracing::debug!("User data: {:#?}", user);

    let mut session = Session::new();

    session
        .insert("user", &user)
        .context("Failed to insert user into session")?;

    let cookie = store
        .store_session(session)
        .await
        .context("Failed to store session")?
        .context("Failed to retrieve session cookie")?;

    let cookie = format!("{COOKIE_NAME}={cookie}; Path=/; Secure; HttpOnly; SameSite=Lax");
    let mut headers = HeaderMap::new();

    headers.insert(
        header::SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    Ok((headers, Redirect::to("/")))
}

async fn logout(
    State(store): State<MemoryStore>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<Response, AppError> {
    let cookie = cookies.get(COOKIE_NAME).context("Missing cookie")?;

    let session = match store
        .load_session(cookie.to_string())
        .await
        .context("Failed to load session")?
    {
        Some(s) => s,
        None => return Ok(Redirect::to("/").into_response()),
    };

    store
        .destroy_session(session)
        .await
        .context("Failed to destroy session")?;

    // Remove the cookie from the response
    let cookie = format!("{COOKIE_NAME}=; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/");
    let mut headers = HeaderMap::new();

    headers.insert(
        header::SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    let mut response = Redirect::to("/").into_response();

    response.headers_mut().extend(headers);

    Ok(response)
}

async fn protected(user: User) -> impl IntoResponse {
    format!(
        "Hey {}! You're logged in!\nYou may now access protected resources.",
        user.name
    )
}

struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/auth/github").into_response()
    }
}

impl<S> FromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthRedirect;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = MemoryStore::from_ref(state);

        let cookies = parts
            .extract::<TypedHeader<headers::Cookie>>()
            .await
            .map_err(|e| match *e.name() {
                header::COOKIE => match e.reason() {
                    TypedHeaderRejectionReason::Missing => AuthRedirect,
                    _ => panic!("Unexpected rejection getting Cookie headers: {:#?}", e),
                },
                _ => panic!("Unexpected rejection while extracting cookies: {:#?}", e),
            })?;

        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AuthRedirect)?;

        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AuthRedirect)?;

        let user = session.get::<User>("user").ok_or(AuthRedirect)?;

        Ok(user)
    }
}

impl<S> OptionalFromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <User as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(user) => Ok(Some(user)),
            Err(AuthRedirect) => Ok(None),
        }
    }
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("App error: {:#}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}
