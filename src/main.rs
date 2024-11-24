mod config;
mod endpoint;
mod model;
mod worker;

use std::{net::SocketAddr, sync::Arc};

use axum::{routing, Router};
use tokio::{
  net::TcpListener,
  runtime::Builder,
  sync::{mpsc, Mutex},
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt::time::ChronoLocal, EnvFilter};

use crate::{config::Config, model::AppState};

const TIMESTAMP_FMT: &str = "%Y-%m-%dT%H:%M:%S%:z";

fn main() {
  let config = match Config::read() {
    Ok(c) => c,
    Err(err) => {
      eprintln!("config error: {err}");
      return;
    }
  };

  let runtime = match Builder::new_multi_thread()
    .worker_threads(config.pool.threads)
    .enable_all()
    .build()
  {
    Ok(r) => r,
    Err(err) => {
      eprintln!("tokio runtime error: {err}");
      return;
    }
  };

  runtime.block_on(run(config));
}

async fn run(config: Config) {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_env("MSSNGR_LOG"))
    .with_timer(ChronoLocal::new(TIMESTAMP_FMT.to_string()))
    .init();

  let (msg_tx, msg_rx) = mpsc::channel(config.max_queue);
  let (listen_tx, listen_rx) = mpsc::channel(100);

  let state = AppState::new(config.topics, config.forward, msg_tx, listen_tx);

  let app = Router::new()
    .route("/", routing::get(endpoint::index))
    .route("/publish/:topic", routing::post(endpoint::publish))
    .route("/subscribe/:topic", routing::get(endpoint::subscribe))
    .route("/fetch/:topic", routing::get(endpoint::fetch))
    .with_state(state.clone())
    .layer(TraceLayer::new_for_http());

  let listener =
    match TcpListener::bind(SocketAddr::new(config.host, config.port)).await {
      Ok(l) => l,
      Err(err) => {
        tracing::error!("binding listener: {}", err);
        return;
      }
    };

  tracing::info!(
    "node: {} | listening on {}",
    config.id,
    listener.local_addr().unwrap()
  );

  let rx = Arc::new(Mutex::new(msg_rx));
  for _ in 0..config.pool.brokers {
    let rx = rx.clone();
    tokio::spawn(worker::broker(rx, state.clone()));
  }

  tokio::spawn(worker::ws_listener(listen_rx, state.clone()));
  tokio::spawn(worker::cleaner(state, config.persistence));

  if let Err(err) = axum::serve(listener, app).await {
    tracing::error!("serving: {}", err);
  }
}
