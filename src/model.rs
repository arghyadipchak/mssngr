use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use chrono::{DateTime, Local};
use dashmap::DashMap;
use futures_util::stream::{SplitSink, SplitStream};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};
use uuid::Uuid;

use crate::config::Node;

#[derive(Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
  #[default]
  Low,
  Medium,
  High,
}

pub struct SubFilter {
  pub priority: Priority,
}

pub struct Subscriber {
  pub id: Uuid,
  pub ws: Mutex<SplitSink<WebSocket, Message>>,
  pub filter: RwLock<SubFilter>,
}

pub struct Topic {
  pub name: String,
  pub subscribers: Arc<DashMap<Uuid, Subscriber>>,
}

impl Topic {
  fn new(name: String) -> Self {
    Self {
      name,
      subscribers: Arc::new(DashMap::new()),
    }
  }
}

pub struct Event {
  pub id: Uuid,
  pub topic: Arc<Topic>,
  pub content: String,
  pub priority: Priority,
  pub timestamp: DateTime<Local>,
}

pub struct SubListen {
  pub ws: SplitStream<WebSocket>,
  pub topic: Arc<Topic>,
  pub sub_id: Uuid,
}

#[derive(Clone)]
pub struct AppState {
  pub fwd_map: Arc<HashMap<String, Arc<Node>>>,
  pub topics: Arc<HashMap<String, Arc<Topic>>>,
  pub event_tx: Sender<Event>,
  pub listen_tx: Sender<SubListen>,
}

impl AppState {
  pub fn new(
    topics: Vec<String>,
    fwd_nodes: Vec<Node>,
    event_tx: Sender<Event>,
    listen_tx: Sender<SubListen>,
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
      .map(|topic| (topic.clone(), Arc::new(Topic::new(topic))))
      .collect();

    Self {
      fwd_map: Arc::new(fwd_map),
      topics: Arc::new(topics),
      event_tx,
      listen_tx,
    }
  }
}
