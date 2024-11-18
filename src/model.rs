use std::{collections::HashMap, sync::Arc};

use tokio::sync::{broadcast, Mutex};

use crate::config::Node;

#[derive(Clone)]
pub struct AppState {
  pub fwd_map: Arc<HashMap<String, Arc<Node>>>,
  pub topics: Arc<HashMap<String, Arc<Mutex<broadcast::Sender<String>>>>>,
}

impl AppState {
  pub fn new(topics: Vec<String>, fwd_nodes: Vec<Node>) -> Self {
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
      .map(|topic| {
        let (tx, _) = broadcast::channel(1);
        (topic, Arc::new(Mutex::new(tx)))
      })
      .collect();

    Self {
      fwd_map: Arc::new(fwd_map),
      topics: Arc::new(topics),
    }
  }
}
