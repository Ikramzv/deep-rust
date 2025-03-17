use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let serve_dir = ServeDir::new("assets").append_index_html_on_directories(true);

    // let (sender, _) = broadcast::channel::<String>(32);

    let app = Router::new()
        .route_service("/", serve_dir)
        .route("/ws", get(ws_handler));
    // .with_state(sender);

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|ws| handle_ws(ws))
}

async fn handle_ws(mut ws: WebSocket) {
    if let Err(err) = ws.send(Message::Text("Hi from server".into())).await {
        tracing::error!("Error sending message {}", err);
        return;
    }

    let protocol = ws.protocol();

    tracing::debug!("established protocol: {:?}", protocol);

    let (mut sender, mut receiver) = ws.split();

    let mut count = 0;

    loop {
        tokio::select! {
            res = receiver.next() => {
                match res {
                    Some(Ok(msg)) => {
                        println!("received: {:?}", msg);

                        if let Message::Close(frame) = msg {
                            tracing::info!("Client disconnected: {:?}", frame);
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        tracing::error!("Error receiving message: {}", err);
                        break;
                    }
                    None => {
                        tracing::error!("Error: message receiver stream closed");
                        break;

                    }
                }
            },
            res = send_count(&mut sender, &mut count) => {
                if let Err(err) = res {
                    // Break the loop if there is an error
                    tracing::error!("Closing connection due to error: {}", err);
                    break;
                }
            }
        }
    }
}

async fn send_count(
    sender: &mut SplitSink<WebSocket, Message>,
    count: &mut i32,
) -> Result<(), String> {
    if let Err(err) = sender
        .send(Message::Text(format!("{}", count).into()))
        .await
    {
        tracing::error!("Error sending message {} {}", err, count);
        return Err(err.to_string());
    };

    *count += 1;
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(())
}
