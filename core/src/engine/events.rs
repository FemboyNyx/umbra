//! Fan-out of [`CoreEvent`]s to subscribers.

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::api::types::CoreEvent;

/// Cheap-to-clone event hub. The FFI layer keeps exactly one subscriber
/// (`events(sink)` replaces the previous one); tests may add several.
#[derive(Clone, Default)]
pub struct EventHub {
    subs: Arc<Mutex<Vec<UnboundedSender<CoreEvent>>>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a subscriber.
    pub fn subscribe(&self) -> UnboundedReceiver<CoreEvent> {
        let (tx, rx) = unbounded_channel();
        self.subs.lock().expect("event hub poisoned").push(tx);
        rx
    }

    /// Drop all existing subscribers and add a new one.
    pub fn subscribe_exclusive(&self) -> UnboundedReceiver<CoreEvent> {
        let (tx, rx) = unbounded_channel();
        let mut subs = self.subs.lock().expect("event hub poisoned");
        subs.clear();
        subs.push(tx);
        rx
    }

    pub fn emit(&self, ev: CoreEvent) {
        let mut subs = self.subs.lock().expect("event hub poisoned");
        subs.retain(|tx| tx.send(ev.clone()).is_ok());
    }
}
