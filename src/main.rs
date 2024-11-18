mod config;
mod endpoint;
mod model;

use std::net::SocketAddr;

use axum::{routing, Router};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt::time::ChronoLocal, EnvFilter};

use crate::config::Config;
use crate::endpoint::*;
use crate::model::AppState;

const TIMESTAMP_FMT: &str = "%Y-%m-%dT%H:%M:%S%:z";

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_env("MSSNGR_LOG"))
    .with_timer(ChronoLocal::new(TIMESTAMP_FMT.to_string()))
    .init();

  let config = Config::read().unwrap();
  let state = AppState::new(config.topics, config.forward);

  let app = Router::new()
    .route("/", routing::get(index))
    .route("/publish/:topic", routing::post(publish))
    .route("/subscribe/:topic", routing::get(subscribe))
    .with_state(state)
    .layer(TraceLayer::new_for_http());

  let listener =
    match TcpListener::bind(SocketAddr::new(config.host, config.port)).await {
      Ok(l) => l,
      Err(err) => {
        tracing::error!("binding listener: {}", err);
        return;
      }
    };

  tracing::debug!(
    "node: {} listening on {}",
    config.id,
    listener.local_addr().unwrap()
  );

  if let Err(err) = axum::serve(listener, app).await {
    tracing::error!("serving: {}", err);
  }
}
