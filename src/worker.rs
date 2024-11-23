use std::sync::Arc;

use axum::extract::ws::Message;
use futures_util::{stream::FuturesUnordered, SinkExt as _, StreamExt as _};
use tokio::sync::{mpsc::Receiver, Mutex};

use crate::model::{Event, SubListen};

pub async fn broker(rx: Arc<Mutex<Receiver<Event>>>) {
  tracing::info!("broker spawned");

  while let Some(event) = rx.lock().await.recv().await {
    for sub in event.topic.subscribers.iter() {
      let filter = sub.filter.read().await;
      if event.priority < filter.priority {
        tracing::debug!(
          "subscriber skipped :: topic: {} | msg_id: {} | sub_id: {}",
          event.topic.name,
          event.id,
          sub.id
        );
        continue;
      }

      if let Err(err) = sub
        .ws
        .lock()
        .await
        .send(Message::Text(event.content.clone()))
        .await
      {
        tracing::info!(
          "subscriber notified :: topic: {} | msg_id: {} | sub_id: {} | error: {}",
          event.topic.name,
          event.id,
          sub.id,
          err
        );
      } else {
        tracing::info!(
          "subscriber notification :: topic: {} | msg_id: {} | sub_id: {}",
          event.topic.name,
          event.id,
          sub.id
        );
      }
    }
  }
}

pub async fn sub_listener(mut rx: Receiver<SubListen>) {
  tracing::info!("websocket listener spawned");

  let mut fut = FuturesUnordered::new();

  if let Some(sub_listen) = rx.recv().await {
    tracing::info!(
      "subscriber listener registered :: topic: {} | id: {}",
      sub_listen.topic.name,
      sub_listen.sub_id
    );

    fut.push(handle_sub_listen(sub_listen));
  } else {
    tracing::info!("no subscribers registered");

    return;
  }

  while let Some(opt) = fut.next().await {
    if let Some(sub_listen) = opt {
      fut.push(handle_sub_listen(sub_listen));
    }
  }
}

async fn handle_sub_listen(mut sub_listen: SubListen) -> Option<SubListen> {
  let (res, ws_rx) = sub_listen.ws.into_future().await;

  if let Some(msg) = res {
    match msg {
      Ok(Message::Close(_)) => {
        sub_listen.topic.subscribers.remove(&sub_listen.sub_id);
        tracing::info!("subscriber closed :: id: {}", &sub_listen.sub_id);
      }
      Ok(_) => tracing::debug!("subscriber message: unknown"),
      Err(err) => tracing::error!("subscriber message: {err}"),
    }

    sub_listen.ws = ws_rx;
    Some(sub_listen)
  } else {
    None
  }
}
