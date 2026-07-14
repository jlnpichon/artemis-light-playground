use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use artemis_light::types::Observer;

/// An artemis-light observer that counts processed events.
///
/// The count is reported by [`run_engine`] on shutdown.
#[derive(Clone)]
pub struct Telemetry {
    pub events: Arc<AtomicU64>,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            events: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<E: Send + 'static> Observer<E, ()> for Telemetry {
    async fn observe_event(&mut self, _event: E) {
        let _ = self.events.fetch_add(1, Ordering::Relaxed);
    }
}
