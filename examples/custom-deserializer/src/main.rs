use std::str::FromStr;

use axum::{extract::Query, response::IntoResponse, routing::get, Router};
use serde::{de, Deserialize, Deserializer};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:3000").await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    Router::new().route("/", get(index))
}

async fn index(Query(params): Query<Params>) -> impl IntoResponse {
    format!("{params:?}")
}

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct Params {
    #[serde(default, deserialize_with = "deserialize_foo")]
    foo: Option<i32>,
    bar: Option<String>,
}

fn deserialize_foo<'de, D>(de: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(de)?;
    match value.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn test_query_params() {
        assert_eq!(
            send_request_get_body("foo=").await,
            r#"Params { foo: None, bar: None }"#
        );

        assert_eq!(
            send_request_get_body("").await,
            r#"Params { foo: None, bar: None }"#
        );

        assert_eq!(
            send_request_get_body("foo=5&bar=bar").await,
            r#"Params { foo: Some(5), bar: Some("bar") }"#
        );
    }

    async fn send_request_get_body(query: &str) -> String {
        let body = app()
            .oneshot(
                Request::builder()
                    .uri(format!("/?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body();

        let bytes = body.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
