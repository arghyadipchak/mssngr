use std::sync::Arc;

use axum::extract::ws::Message;
use chrono::{DateTime, Local};
use futures_util::{stream::FuturesUnordered, SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::Receiver, Mutex};
use uuid::Uuid;

use crate::model::{AppState, ListenEvent, Mode, MsgEvent, Priority};

#[derive(Serialize)]
struct PullData {
  pub id: Uuid,
  pub timestamp: DateTime<Local>,
}

pub async fn broker(rx: Arc<Mutex<Receiver<MsgEvent>>>, state: AppState) {
  tracing::info!("broker spawned");

  while let Some(event) = rx.lock().await.recv().await {
    let topic = if let Some(topic) = state.topics.get(&event.topic) {
      topic
    } else {
      continue;
    };

    for sub in topic.subscribers.iter() {
      let meta = sub.meta.read().await;
      if event.priority < meta.priority {
        tracing::debug!(
          "subscriber skipped :: topic: {} | msg_id: {} | sub_id: {}",
          event.topic,
          event.id,
          sub.id
        );
        continue;
      }

      let msg = match meta.mode {
        Mode::Push => serde_json::to_string(&event),
        Mode::Pull => serde_json::to_string(&PullData {
          id: event.id,
          timestamp: event.timestamp,
        }),
      };
      let msg_text = match msg {
        Ok(m) => Message::Text(m),
        Err(err) => {
          tracing::error!(
            "subscriber notification serialization :: topic: {} | msg_id: {} | sub_id: {} | error: {}",
            event.topic,
            event.id,
            sub.id,
            err
          );
          continue;
        }
      };

      if let Err(err) = sub.ws.lock().await.send(msg_text).await {
        tracing::info!(
          "subscriber notified :: topic: {} | msg_id: {} | sub_id: {} | error: {}",
          event.topic,
          event.id,
          sub.id,
          err
        );
      } else {
        tracing::info!(
          "subscriber notification :: topic: {} | msg_id: {} | sub_id: {}",
          event.topic,
          event.id,
          sub.id
        );
      }
    }
  }
}

pub async fn ws_listener(mut rx: Receiver<ListenEvent>, state: AppState) {
  tracing::info!("websocket listener spawned");

  let mut fut = FuturesUnordered::new();

  if let Some(sub_listen) = rx.recv().await {
    tracing::info!(
      "subscriber listener registered :: topic: {} | id: {}",
      sub_listen.topic,
      sub_listen.sub_id
    );

    fut.push(handle_listen(sub_listen, state.clone()));
  } else {
    tracing::info!("no subscribers registered");

    return;
  }

  while let Some(opt) = fut.next().await {
    if let Some(sub_listen) = opt {
      fut.push(handle_listen(sub_listen, state.clone()));
    }
  }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum WsMessage {
  Update {
    mode: Option<Mode>,
    priority: Option<Priority>,
  },
}

async fn handle_listen(
  mut listen_event: ListenEvent,
  state: AppState,
) -> Option<ListenEvent> {
  let (res, ws_rx) = listen_event.ws.into_future().await;

  if let Some(msg) = res {
    let topic = state.topics.get(&listen_event.topic)?;

    match msg {
      Ok(Message::Close(_)) => {
        topic.subscribers.remove(&listen_event.sub_id);
        tracing::info!("subscriber closed :: id: {}", &listen_event.sub_id);
      }
      Ok(Message::Text(txt)) => {
        match serde_json::from_str::<WsMessage>(&txt) {
          Ok(WsMessage::Update { mode, priority }) => {
            let sub = topic.subscribers.get(&listen_event.sub_id)?;
            let mut meta = sub.meta.write().await;

            if let Some(mode) = mode {
              meta.mode = mode;
            }
            if let Some(priority) = priority {
              meta.priority = priority;
            }

            tracing::info!(
              "subscriber meta updated :: id: {}",
              listen_event.sub_id
            );
          }
          _ => {
            tracing::debug!(
              "websocket unknown message :: sub_id: {}",
              listen_event.sub_id
            );
          }
        };
      }
      Ok(_) => tracing::debug!("subscriber message: unknown"),
      Err(err) => tracing::error!("subscriber message: {err}"),
    }

    listen_event.ws = ws_rx;
    Some(listen_event)
  } else {
    None
  }
}
