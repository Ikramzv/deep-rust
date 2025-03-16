use std::sync::{Arc, LazyLock};

use axum::{extract::State, http::StatusCode, response::Html, routing::get, Router};
use minijinja::{context, Environment};
use serde::Serialize;
use tokio::net::TcpListener;

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
    let mut env = Environment::new();

    env.add_template("layout", include_str!("../templates/layout.jinja"))
        .unwrap();

    env.add_template("home", include_str!("../templates/home.jinja"))
        .unwrap();

    env.add_template("content", include_str!("../templates/content.jinja"))
        .unwrap();

    env.add_template("about", include_str!("../templates/about.jinja"))
        .unwrap();

    let app_state = Arc::new(AppState { env });

    let app = Router::new()
        .route("/", get(home_page))
        .route("/about", get(about_page))
        .route("/content", get(content_page))
        .with_state(app_state);

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

async fn home_page(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, String)> {
    println!("home_page");
    let template = state.env.get_template("home").unwrap();

    println!("template: {:?}", template);

    let links = LINKS.clone();

    let rendered = template
        .render(context! {
            title => "Home",
            welcome_text => "Welcome to the home page",
            links => links
        })
        .inspect_err(|e| println!("error: {:?}", e))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Html(rendered))
}

async fn about_page(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, String)> {
    let template = state.env.get_template("about").unwrap();

    let links = LINKS.clone();

    let rendered = template
        .render(context! {
            title => "About",
            content => "This is the about page",
            links => links
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Html(rendered))
}

async fn content_page(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, String)> {
    let template = state.env.get_template("content").unwrap();

    let links = LINKS.clone();

    let entries = vec![
        Entry {
            name: "Entry 1".to_string(),
        },
        Entry {
            name: "Entry 2".to_string(),
        },
        Entry {
            name: "Entry 3".to_string(),
        },
        Entry {
            name: "Entry 4".to_string(),
        },
    ];

    let rendered = template
        .render(context! {
            title => "Content",
            links => links,
            entries => entries,
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Html(rendered))
}

#[derive(Debug)]
struct AppState {
    env: Environment<'static>,
}

#[derive(Debug, Serialize, Clone)]
struct Link {
    title: String,
    href: String,
}

#[derive(Debug, Serialize)]
struct Entry {
    name: String,
}
