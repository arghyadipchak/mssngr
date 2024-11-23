use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use chrono::{DateTime, Local};
use futures::stream::SplitSink;
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
  pub ws: Mutex<SplitSink<WebSocket, Message>>,
  pub filter: RwLock<SubFilter>,
}

#[derive(Default)]
pub struct Topic {
  pub subscribers: Arc<RwLock<Vec<Subscriber>>>,
}

#[derive(Serialize)]
pub struct Event {
  pub id: Uuid,
  #[serde(skip_serializing)]
  pub topic: Arc<Topic>,
  pub content: String,
  pub priority: Priority,
  pub timestamp: DateTime<Local>,
}

#[derive(Clone)]
pub struct AppState {
  pub fwd_map: Arc<HashMap<String, Arc<Node>>>,
  pub topics: Arc<HashMap<String, Arc<Topic>>>,
  pub event_tx: Sender<Event>,
}

impl AppState {
  pub fn new(
    topics: Vec<String>,
    fwd_nodes: Vec<Node>,
    event_tx: Sender<Event>,
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
      .map(|topic| (topic, Arc::new(Topic::default())))
      .collect();

    Self {
      fwd_map: Arc::new(fwd_map),
      topics: Arc::new(topics),
      event_tx,
    }
  }
}
