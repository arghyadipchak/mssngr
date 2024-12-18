use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use chrono::{DateTime, Local};
use dashmap::DashMap;
use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};
use uuid::Uuid;

use crate::config::Node;

#[derive(
  Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
  #[default]
  Low,
  Medium,
  High,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
  #[default]
  Push,
  Pull,
}

#[derive(Deserialize)]
pub struct Meta {
  #[serde(default)]
  pub mode: Mode,

  #[serde(default)]
  pub priority: Priority,
}

pub struct Subscriber {
  pub id: Uuid,
  pub meta: RwLock<Meta>,
  pub ws: Mutex<SplitSink<WebSocket, Message>>,
}

impl Subscriber {
  pub fn new(meta: Meta, ws_tx: SplitSink<WebSocket, Message>) -> Self {
    Self {
      id: Uuid::new_v4(),
      meta: RwLock::new(meta),
      ws: Mutex::new(ws_tx),
    }
  }
}

#[derive(Default)]
pub struct Topic {
  pub subscribers: DashMap<Uuid, Subscriber>,
  pub msg_events: DashMap<Uuid, MsgEvent>,
}

#[derive(Clone, Serialize)]
pub struct MsgEvent {
  pub id: Uuid,
  pub topic: String,
  pub content: String,
  pub priority: Priority,
  pub timestamp: DateTime<Local>,
}

pub struct ListenEvent {
  pub ws: SplitStream<WebSocket>,
  pub topic: String,
  pub sub_id: Uuid,
}

#[derive(Clone)]
pub struct AppState {
  pub fwd_map: Arc<HashMap<String, Arc<Node>>>,
  pub topics: Arc<HashMap<String, Topic>>,
  pub msg_tx: Sender<MsgEvent>,
  pub listen_tx: Sender<ListenEvent>,
}

impl AppState {
  pub fn new(
    topics: Vec<String>,
    fwd_nodes: Vec<Node>,
    msg_tx: Sender<MsgEvent>,
    listen_tx: Sender<ListenEvent>,
  ) -> Self {
    let fwd_map = fwd_nodes
      .into_iter()
      .flat_map(|node| {
        let node = Arc::new(node);
        node
          .topics
          .clone()
          .into_iter()
          .map(move |topic| (topic, node.clone()))
      })
      .collect();

    let topics = topics
      .into_iter()
      .map(|topic| (topic, Topic::default()))
      .collect();

    Self {
      fwd_map: Arc::new(fwd_map),
      topics: Arc::new(topics),
      msg_tx,
      listen_tx,
    }
  }
}
