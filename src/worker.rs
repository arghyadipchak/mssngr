use std::{sync::Arc, time::Duration};

use axum::extract::ws::Message;
use chrono::{DateTime, Local};
use futures_util::{stream::FuturesUnordered, SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio::{
  sync::{mpsc::Receiver, Mutex},
  time,
};
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
    let topic_state = if let Some(t) = state.topics.get(&event.topic) {
      t
    } else {
      continue;
    };

    let push_txt = serde_json::to_string(&event).unwrap_or_default();
    let pull_txt = serde_json::to_string(&PullData {
      id: event.id,
      timestamp: event.timestamp,
    })
    .unwrap_or_default();

    topic_state.msg_events.insert(event.id, event.clone());

    for sub in topic_state.subscribers.iter() {
      let meta = sub.meta.read().await;
      if event.priority < meta.priority {
        tracing::debug!(
          "subscriber skipped :: topic: {} | sub_id: {} | msg_id: {}",
          event.topic,
          sub.id,
          event.id
        );
        continue;
      }

      let msg = Message::Text(match sub.meta.read().await.mode {
        Mode::Push => push_txt.clone(),
        Mode::Pull => pull_txt.clone(),
      });

      if let Err(err) = sub.ws.lock().await.send(msg).await {
        tracing::error!(
            "subscriber notify :: topic: {} | sub_id: {} | msg_id: {} | error: {}",
            event.topic,
            sub.id,
            event.id,
            err
          );
      } else {
        tracing::info!(
          "subscriber notify :: topic: {} | sub_id: {} | msg_id: {}",
          event.topic,
          sub.id,
          event.id
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
  Fetch {
    id: Uuid,
  },
}

async fn handle_listen(
  mut listen_event: ListenEvent,
  state: AppState,
) -> Option<ListenEvent> {
  let (res, ws_rx) = listen_event.ws.into_future().await;

  if let Some(msg) = res {
    let topic_state = state.topics.get(&listen_event.topic)?;

    match msg {
      Ok(Message::Close(_)) => {
        topic_state.subscribers.remove(&listen_event.sub_id);
        tracing::info!("subscriber closed :: id: {}", &listen_event.sub_id);
      }
      Ok(Message::Text(txt)) => {
        match serde_json::from_str::<WsMessage>(&txt) {
          Ok(WsMessage::Update { mode, priority }) => {
            let sub = topic_state.subscribers.get(&listen_event.sub_id)?;
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
          Ok(WsMessage::Fetch { id }) => {
            if let Some(msg_event) = topic_state.msg_events.get(&id) {
              if let Some(sub) =
                topic_state.subscribers.get(&listen_event.sub_id)
              {
                let msg = Message::Text(
                  serde_json::to_string(&msg_event.clone()).unwrap_or_default(),
                );

                if let Err(err) = sub.ws.lock().await.send(msg).await {
                  tracing::error!(
                    "subscriber fetch :: topic: {} | sub_id: {} | msg_id: {} | error: {}",
                    msg_event.topic,
                    sub.id,
                    id,
                    err
                  );
                } else {
                  tracing::info!(
                    "subscriber fetch :: topic: {} | sub_id: {} | msg_id: {}",
                    msg_event.topic,
                    sub.id,
                    id
                  );
                }
              }
            } else {
              tracing::debug!(
                "subscriber fetch message not found :: topic: {} | sub_id: {} | msg_id: {}",
                listen_event.topic,
                listen_event.sub_id,
                id
              );
            }
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

pub async fn cleaner(state: AppState, interval: Duration) {
  loop {
    time::sleep(interval).await;
    let now = Local::now();

    for (topic, topic_state) in state.topics.iter() {
      tracing::info!("cleaning :: topic: {}", topic);

      topic_state.msg_events.retain(|_, msg_event| {
        (now - msg_event.timestamp).to_std().unwrap_or_default() < interval
      });
    }
  }
}
