use axum::{
  extract::{ws::WebSocket, Path, Query, State, WebSocketUpgrade},
  http::StatusCode,
  response::{IntoResponse, Redirect, Response},
  Json,
};
use chrono::Local;
use futures_util::StreamExt;
use serde::Deserialize;

use uuid::Uuid;

use crate::model::{
  AppState, ListenEvent, Meta, MsgEvent, Priority, Subscriber,
};

#[axum::debug_handler]
pub async fn index() -> StatusCode {
  StatusCode::OK
}

#[derive(Deserialize)]
pub struct Payload {
  topic: String,
  content: String,

  #[serde(default)]
  priority: Priority,
}

impl From<Payload> for MsgEvent {
  fn from(payload: Payload) -> Self {
    Self {
      id: Uuid::new_v4(),
      topic: payload.topic,
      content: payload.content,
      priority: payload.priority,
      timestamp: Local::now(),
    }
  }
}

#[axum::debug_handler]
pub async fn publish(
  State(state): State<AppState>,
  Json(payload): Json<Payload>,
) -> Response {
  let topic = payload.topic.clone();

  if state.topics.contains_key(&topic) {
    let event = MsgEvent::from(payload);
    let id = event.id;

    return match state.msg_event_tx.send(event).await {
      Ok(()) => {
        tracing::info!("message queued :: topic: {} | id: {}", topic, id);
        StatusCode::CREATED
      }
      Err(err) => {
        tracing::error!(
          "message queue :: topic: {} | id: {} | error: {}",
          topic,
          id,
          err
        );
        StatusCode::INTERNAL_SERVER_ERROR
      }
    }
    .into_response();
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!(
      "redirection :: topic: {} | node: {} ({})",
      topic,
      node.id,
      node.addr
    );

    return Redirect::temporary(&format!("{}publish", node.addr))
      .into_response();
  }

  tracing::debug!("not found :: topic: {}", topic);
  StatusCode::BAD_REQUEST.into_response()
}

#[axum::debug_handler]
pub async fn subscribe(
  Path(topic): Path<String>,
  Query(meta): Query<Meta>,
  State(state): State<AppState>,
  ws: WebSocketUpgrade,
) -> Response {
  if state.topics.contains_key(&topic) {
    tracing::info!("subcriber connection :: topic: {}", topic);

    return ws
      .on_upgrade(|socket| handle_ws(socket, meta, topic, state))
      .into_response();
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!(
      "redirection :: topic: {} | node: {} ({})",
      topic,
      node.id,
      node.addr
    );

    return Redirect::temporary(&format!("{}subscribe/{topic}", node.addr))
      .into_response();
  }

  tracing::debug!("not found :: topic: {}", topic);
  StatusCode::BAD_GATEWAY.into_response()
}

async fn handle_ws(
  socket: WebSocket,
  meta: Meta,
  topic: String,
  state: AppState,
) {
  let (ws_tx, ws_rx) = socket.split();

  let sub = Subscriber::new(meta, ws_tx);
  let sub_listen = ListenEvent {
    ws: ws_rx,
    topic: topic.clone(),
    sub_id: sub.id,
  };

  match state.listen_event_tx.send(sub_listen).await {
    Ok(()) => {
      tracing::info!(
        "subscriber registered :: topic: {} | id: {}",
        topic,
        sub.id
      );
      if let Some(topic_state) = state.topics.get(&topic) {
        topic_state.subscribers.insert(sub.id, sub);
      }
    }
    Err(err) => {
      tracing::info!(
        "subscriber registration :: topic: {} | id: {} | error: {}",
        topic,
        sub.id,
        err
      );
    }
  }
}
