use axum::{
  extract::{Path, State, WebSocketUpgrade},
  http::StatusCode,
  response::{IntoResponse, Redirect, Response},
  Json,
};
use chrono::Local;
use futures::StreamExt as _;
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::model::{AppState, Event, Priority, SubFilter, Subscriber};

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
    let event = Event {
      id: Uuid::new_v4(),
      topic: tp.clone(),
      content: payload.content,
      priority: payload.priority,
      timestamp: tstamp,
    };

    return match state.event_tx.send(event).await {
      Ok(()) => {
        tracing::info!("broadcast :: topic: {}", payload.topic);
        StatusCode::CREATED
      }
      Err(err) => {
        tracing::error!("broadcast :: topic: {} error: {}", payload.topic, err);
        StatusCode::INTERNAL_SERVER_ERROR
      }
    }
    .into_response();
  }

  if let Some(node) = state.fwd_map.get(&payload.topic) {
    tracing::info!("redirection :: node: {} ({})", node.id, node.addr);

    return Redirect::temporary(&format!("{}publish", node.addr))
      .into_response();
  }

  StatusCode::BAD_REQUEST.into_response()
}

#[axum::debug_handler]
pub async fn subscribe(
  Path(topic): Path<String>,
  State(state): State<AppState>,
  ws: WebSocketUpgrade,
) -> Response {
  if let Some(tp) = state.topics.get(&topic) {
    tracing::info!("subscribed :: topic: {}", topic);

    let tp = tp.clone();
    return ws
      .on_upgrade(|socket| async move {
        let (tx, _) = socket.split();

        let sub = Subscriber {
          ws: Mutex::new(tx),
          filter: RwLock::new(SubFilter {
            priority: Priority::default(),
          }),
        };
        tp.subscribers.write().await.push(sub);
      })
      .into_response();
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!("redirection :: node: {} ({})", node.id, node.addr);

    return Redirect::temporary(&format!("{}subscribe/{topic}", node.addr))
      .into_response();
  }

  StatusCode::BAD_GATEWAY.into_response()
}
