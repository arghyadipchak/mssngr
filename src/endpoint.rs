use axum::{
  extract::{Path, State},
  http::StatusCode,
  response::{IntoResponse, Redirect, Response},
  Json,
};
use serde::Deserialize;

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
  if let Some(queue) = state.msg_queue.get(&topic) {
    match queue.lock() {
      Ok(mut queue) => {
        queue.push(payload.content);
        tracing::info!("msg pushed to queue: {}", topic);

        return StatusCode::CREATED.into_response();
      }
      Err(err) => {
        tracing::error!("queue lock: {}", err);

        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
      }
    }
  };

  for (addr, topics) in state.redir_map.iter() {
    if topics.contains(&topic) {
      return Redirect::temporary(&format!("http://{addr}/publish/{topic}",))
        .into_response();
    }
  }

  StatusCode::BAD_GATEWAY.into_response()
}
