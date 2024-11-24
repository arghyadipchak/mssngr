use std::{fmt::Display, str::FromStr};

use axum::{
  extract::{ws::WebSocket, Path, Query, State, WebSocketUpgrade},
  http::StatusCode,
  response::{IntoResponse, Redirect, Response},
  Json,
};
use chrono::Local;
use futures_util::StreamExt;
use serde::{de, Deserialize, Deserializer};
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
  content: String,

  #[serde(default)]
  priority: Priority,
}

#[axum::debug_handler]
pub async fn publish(
  Path(topic): Path<String>,
  State(state): State<AppState>,
  Json(payload): Json<Payload>,
) -> Response {
  if state.topics.contains_key(&topic) {
    let id = Uuid::new_v4();
    let event = MsgEvent {
      id,
      topic: topic.clone(),
      content: payload.content,
      priority: payload.priority,
      timestamp: Local::now(),
    };

    return match state.msg_tx.send(event).await {
      Ok(()) => {
        tracing::info!("message queued :: topic: {} | id: {}", topic, id);
        (StatusCode::CREATED, Json(serde_json::json!({ "id": id })))
          .into_response()
      }
      Err(err) => {
        tracing::error!(
          "message queue :: topic: {} | id: {} | error: {}",
          topic,
          id,
          err
        );
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
      }
    };
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!(
      "redirection :: topic: {} | node: {} ({})",
      topic,
      node.id,
      node.addr
    );

    return Redirect::temporary(&format!("{}publish/{topic}", node.addr))
      .into_response();
  }

  tracing::debug!("not found :: topic: {}", topic);
  StatusCode::BAD_GATEWAY.into_response()
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

  match state.listen_tx.send(sub_listen).await {
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

#[derive(Deserialize)]
pub struct FetchQuery {
  #[serde(deserialize_with = "deserialize_vec")]
  id: Vec<Uuid>,
}

fn deserialize_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
  D: Deserializer<'de>,
  T: FromStr,
  T::Err: Display,
{
  String::deserialize(deserializer)?
    .split(',')
    .map(|item| item.trim().parse().map_err(de::Error::custom))
    .collect()
}

#[axum::debug_handler]
pub async fn fetch(
  Path(topic): Path<String>,
  Query(fq): Query<FetchQuery>,
  State(state): State<AppState>,
) -> Response {
  if let Some(topic_state) = state.topics.get(&topic) {
    tracing::info!("subcriber fetch :: topic: {}", topic);

    let resp = fq
      .id
      .iter()
      .filter_map(|id| {
        topic_state.msg_events.get(id).map(|x| x.value().clone())
      })
      .collect::<Vec<_>>();

    return Json(resp).into_response();
  }

  if let Some(node) = state.fwd_map.get(&topic) {
    tracing::info!(
      "redirection :: topic: {} | node: {} ({})",
      topic,
      node.id,
      node.addr
    );

    return Redirect::temporary(&format!("{}fetch/{topic}", node.addr))
      .into_response();
  }

  tracing::debug!("not found :: topic: {}", topic);
  StatusCode::BAD_GATEWAY.into_response()
}
