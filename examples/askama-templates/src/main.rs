use std::sync::LazyLock;

use askama::Template;
use axum::{http::StatusCode, response::Html, routing::get, Router};
use serde::Serialize;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const LINKS: LazyLock<Vec<Link>> = LazyLock::new(|| {
    vec![
        Link {
            title: "Home".to_string(),
            href: "/".to_string(),
        },
        Link {
            title: "About".to_string(),
            href: "/about".to_string(),
        },
        Link {
            title: "Content".to_string(),
            href: "/content".to_string(),
        },
    ]
});

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug, info, error".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new().route("/", get(home_page));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

async fn home_page() -> Result<Html<String>, (StatusCode, String)> {
    let links = LINKS.clone();

    let template = HomeTemplate {
        title: "Home".to_string(),
        welcome_text: "Welcome to the home page".to_string(),
        links,
    };

    match template.render() {
        Ok(html) => Ok(Html(html)),
        Err(e) => {
            tracing::error!("Error rendering template: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".into(),
            ))
        }
    }
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    title: String,
    welcome_text: String,
    links: Vec<Link>,
}

#[derive(Debug, Serialize, Clone)]
struct Link {
    title: String,
    href: String,
}
