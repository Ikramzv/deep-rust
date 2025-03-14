use axum::{
    body::Body,
    extract::{Request, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use hyper::{StatusCode, Uri};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

const PROXY_PORT: u16 = 4000;
const PORT: u16 = 3000;

#[tokio::main]
async fn main() {
    tokio::spawn(server());

    let client: Client = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
        .build(HttpConnector::new());

    let app = Router::new()
        .without_v07_checks()
        .route("/*", get(handler))
        .with_state(client);

    let listener = tokio::net::TcpListener::bind(format!("localhost:{}", PROXY_PORT))
        .await
        .unwrap();

    println!(
        "Reverse proxy listening on {}",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}

async fn handler(State(client): State<Client>, mut req: Request) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    let uri = format!("http://localhost:{}{}", PORT, path_query);

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    Ok(client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}

async fn server() {
    async fn index() -> &'static str {
        "Hello, World!"
    }

    let app = Router::new().route("/", get(index));

    let listener = tokio::net::TcpListener::bind(format!("localhost:{}", PORT))
        .await
        .unwrap();

    println!("Server listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
