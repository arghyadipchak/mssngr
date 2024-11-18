use axum::{
  extract::{
    ws::{Message, WebSocket},
    Path, State, WebSocketUpgrade,
  },
  http::StatusCode,
  response::{IntoResponse, Redirect, Response},
  Json,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::model::AppState;

#[axum::debug_handler]
pub async fn index() -> StatusCode {
  StatusCode::OK
}

#[derive(Debug, Deserialize)]
pub struct PublishData {
  content: String,
}

#[axum::debug_handler]
pub async fn publish(
  Path(topic): Path<String>,
  State(state): State<AppState>,
  Json(payload): Json<PublishData>,
) -> Response {
  if let Some(txl) = state.topics.get(&topic) {
    let tx = txl.lock().await;

    return match tx.send(payload.content) {
      Ok(_) => {
        tracing::info!("broadcast :: topic: {}", topic);
        StatusCode::CREATED
      }
      Err(err) => {
        tracing::error!("broadcast :: topic: {} error: {}", topic, err);
        StatusCode::INTERNAL_SERVER_ERROR
      }
    }
    .into_response();
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!("redirection :: node: {} ({})", node.id, node.addr);

    return Redirect::temporary(&format!("{}publish/{topic}", node.addr))
      .into_response();
  }

  StatusCode::BAD_GATEWAY.into_response()
}

#[axum::debug_handler]
pub async fn subscribe(
  Path(topic): Path<String>,
  State(state): State<AppState>,
  ws: WebSocketUpgrade,
) -> Response {
  if let Some(txl) = state.topics.get(&topic) {
    let tx = txl.lock().await;
    let rx = tx.subscribe();

    tracing::info!("subscribed :: topic: {}", topic);

    return ws
      .on_upgrade(move |socket| handle_ws(socket, topic, rx))
      .into_response();
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!("redirection :: node: {} ({})", node.id, node.addr);

    return Redirect::temporary(&format!("{}subscribe/{topic}", node.addr))
      .into_response();
  }

  StatusCode::BAD_GATEWAY.into_response()
}

async fn handle_ws(
  socket: WebSocket,
  topic: String,
  mut rx: broadcast::Receiver<String>,
) {
  let (mut ws_tx, _) = socket.split();

  loop {
    match rx.recv().await {
      Ok(content) => {
        if let Err(err) = ws_tx.send(Message::Text(content)).await {
          tracing::error!("ws_sender :: topic: {} error: {}", topic, err);
          break;
        }
      }
      Err(err) => {
        tracing::error!("receiver :: topic: {} error: {}", topic, err);
        break;
      }
    }
  }
}
