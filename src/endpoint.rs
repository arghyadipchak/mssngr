use std::sync::Arc;

use axum::{
  extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
  http::StatusCode,
  response::{IntoResponse, Redirect, Response},
  Json,
};
use chrono::Local;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::{mpsc::Sender, Mutex, RwLock};
use uuid::Uuid;

use crate::model::{
  AppState, Event, Priority, SubFilter, SubListen, Subscriber, Topic,
};

#[axum::debug_handler]
pub async fn index() -> StatusCode {
  StatusCode::OK
}

#[derive(Deserialize)]
pub struct Payload {
  pub topic: String,
  pub content: String,

  #[serde(default)]
  pub priority: Priority,
}

#[axum::debug_handler]
pub async fn publish(
  State(state): State<AppState>,
  Json(payload): Json<Payload>,
) -> Response {
  let tstamp = Local::now();

  if let Some(tp) = state.topics.get(&payload.topic) {
    let id = Uuid::new_v4();
    let event = Event {
      id,
      topic: tp.clone(),
      content: payload.content,
      priority: payload.priority,
      timestamp: tstamp,
    };

    return match state.event_tx.send(event).await {
      Ok(()) => {
        tracing::info!(
          "message queued :: topic: {} | id: {}",
          payload.topic,
          id
        );
        StatusCode::CREATED
      }
      Err(err) => {
        tracing::error!(
          "message queue :: topic: {} | id: {} | error: {}",
          payload.topic,
          id,
          err
        );
        StatusCode::INTERNAL_SERVER_ERROR
      }
    }
    .into_response();
  }

  if let Some(node) = state.fwd_map.get(&payload.topic) {
    tracing::info!(
      "redirection :: topic: {} | node: {} ({})",
      payload.topic,
      node.id,
      node.addr
    );

    return Redirect::temporary(&format!("{}publish", node.addr))
      .into_response();
  }

  tracing::debug!("not found :: topic: {}", payload.topic);
  StatusCode::BAD_REQUEST.into_response()
}

#[axum::debug_handler]
pub async fn subscribe(
  Path(topic): Path<String>,
  State(state): State<AppState>,
  ws: WebSocketUpgrade,
) -> Response {
  if let Some(topic_state) = state.topics.get(&topic) {
    tracing::info!("subcriber connection :: topic: {}", topic);

    let topic = topic_state.clone();
    return ws
      .on_upgrade(|socket| handle_ws(socket, topic, state.listen_tx))
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
  topic: Arc<Topic>,
  tx: Sender<SubListen>,
) {
  let id = Uuid::new_v4();
  let (ws_tx, ws_rx) = socket.split();

  let sub = Subscriber {
    id,
    ws: Mutex::new(ws_tx),
    filter: RwLock::new(SubFilter {
      priority: Priority::default(),
    }),
  };
  let sub_listen = SubListen {
    ws: ws_rx,
    topic: topic.clone(),
    sub_id: id,
  };

  match tx.send(sub_listen).await {
    Ok(()) => {
      tracing::info!(
        "subscriber registered :: topic: {} | id: {}",
        topic.name,
        id
      );
      topic.subscribers.insert(id, sub);
    }
    Err(err) => {
      tracing::info!(
        "subscriber registration :: topic: {} | id: {} | error: {}",
        topic.name,
        id,
        err
      );
    }
  }
}
