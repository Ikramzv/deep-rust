use std::{net::TcpListener, time::Duration};

use axum::{routing::get, Router};
use listenfd::ListenFd;

#[tokio::main]
async fn main() {
    let app: Router<()> = Router::new().route("/", get(handler));

    let mut listenfd = ListenFd::from_env();

    let listener = match listenfd.take_tcp_listener(0).unwrap() {
        Some(listener) => {
            println!("using socket from listenfd");
            tokio::net::TcpListener::from_std(listener).unwrap()
        }
        None => tokio::net::TcpListener::bind("localhost:3000")
            .await
            .unwrap(),
    };

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> &'static str {
    tokio::time::sleep(Duration::from_secs(4)).await;
    "Hello, wold"
}
