use std::sync::Arc;

use axum::extract::ws::Message;
use futures::SinkExt as _;
use tokio::sync::{mpsc::Receiver, Mutex};

use crate::model::Event;

pub async fn broker(rx: Arc<Mutex<Receiver<Event>>>) {
  while let Some(event) = rx.lock().await.recv().await {
    for sub in event.topic.subscribers.read().await.iter() {
      let filter = sub.filter.read().await;
      if event.priority < filter.priority {
        continue;
      }

      if let Err(err) = sub
        .ws
        .lock()
        .await
        .send(Message::Text(event.content.clone()))
        .await
      {
        tracing::error!("subscriber event :: {err}");
      }
    }
  }
}
