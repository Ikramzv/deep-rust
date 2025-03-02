use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{
        ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};

use futures::{SinkExt, StreamExt};
use tokio::{net::TcpListener, sync::broadcast};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    users: Mutex<HashSet<String>>,
    // channel used to send messages to all users
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (tx, _rx) = broadcast::channel(16);

    let app_state = Arc::new(AppState {
        users: Mutex::new(HashSet::new()),
        tx,
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(websocket_handler))
        .with_state(app_state);

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap()
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = stream.split();

    println!("WebSocket connected");

    let mut username = String::new();

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {
            tracing::debug!("Received username: {name}");

            check_username(&state, &mut username, name.as_str());

            if !username.is_empty() {
                break;
            } else {
                println!("Username already taken");

                let _ = sender
                    .send(Message::text(Utf8Bytes::from_static(
                        "Username already taken",
                    )))
                    .await;

                return;
            }
        }
    }

    let mut rx = state.tx.subscribe();

    let msg = format!("{username} joined the chat");
    let _ = state.tx.send(msg);

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::text(msg)).await.is_err() {
                break;
            }
        }
    });

    let tx = state.tx.clone();
    let name = username.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            tracing::debug!("Received message: {text}");
            let _ = tx.send(format!("{name}: {text}"));
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    let name = username.clone();

    let msg = format!("{name} left the chat");
    let _ = state.tx.send(msg);

    let mut users = state.users.lock().unwrap();
    users.remove(&name);
    let _ = state.tx.send(format!("{} users in chat", users.len()));
}

fn check_username(state: &AppState, username: &mut String, name: &str) {
    let mut users = state.users.lock().unwrap();

    if !users.contains(name) {
        users.insert(name.to_owned());
        username.push_str(name);
    }
}

async fn index() -> Html<String> {
    Html(include_str!("../index.html").to_string())
}
