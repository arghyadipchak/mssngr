use std::{
  collections::{HashMap, HashSet},
  sync::{Arc, Mutex},
};

use crate::config::Node;

#[derive(Clone)]
pub struct AppState {
  pub redir_map: Arc<Vec<(String, HashSet<String>)>>,
  pub msg_queue: Arc<HashMap<String, Arc<Mutex<Vec<String>>>>>,
}

impl AppState {
  pub fn new(topics: Vec<String>, redir_nodes: Vec<Node>) -> Self {
    Self {
      redir_map: Arc::new(
        redir_nodes
          .into_iter()
          .map(|n| (n.addr, n.topics))
          .collect(),
      ),
      msg_queue: Arc::new(
        topics
          .into_iter()
          .map(|topic| (topic, Arc::new(Mutex::new(Vec::new()))))
          .collect(),
      ),
    }
  }
}
