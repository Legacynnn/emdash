//! `ClassifierRegistry` — maps agent name → boxed `Classifier`.
//!
//! Constructed at startup with the `register(name, classifier)` API;
//! the hook server's request handler calls `for_agent` per request.
//! `parking_lot::RwLock` because reads dominate writes (the registry
//! is bootstrap-time-mutable in practice).

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::classifier::Classifier;

#[derive(Default)]
pub struct ClassifierRegistry {
    by_name: RwLock<HashMap<String, Arc<dyn Classifier>>>,
}

impl ClassifierRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<C: Classifier + 'static>(&self, classifier: C) {
        let name = classifier.name().to_string();
        self.by_name.write().insert(name, Arc::new(classifier));
    }

    pub fn for_agent(&self, agent: &str) -> Option<Arc<dyn Classifier>> {
        self.by_name.read().get(agent).cloned()
    }

    pub fn len(&self) -> usize {
        self.by_name.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.read().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::super::classifier::ClaudeClassifier;
    use super::*;

    #[test]
    fn register_then_for_agent_returns_match() {
        let r = ClassifierRegistry::new();
        r.register(ClaudeClassifier);
        let c = r.for_agent("claude").expect("registered");
        assert_eq!(c.name(), "claude");
    }

    #[test]
    fn for_agent_returns_none_for_unknown() {
        let r = ClassifierRegistry::new();
        r.register(ClaudeClassifier);
        assert!(r.for_agent("nope").is_none());
    }
}
