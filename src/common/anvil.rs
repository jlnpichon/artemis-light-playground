use std::ops::Deref;

use alloy::node_bindings::AnvilInstance;
use tracing::info;

/// A drop guard that logs when the forked Anvil node starts and stops.
pub struct AnvilProcess {
    inner: AnvilInstance,
}

impl AnvilProcess {
    pub fn new(inner: AnvilInstance) -> Self {
        info!(
            pid = inner.child().id(),
            endpoint = %inner.endpoint_url(),
            "anvil forked node started"
        );
        Self { inner }
    }
}

impl Deref for AnvilProcess {
    type Target = AnvilInstance;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for AnvilProcess {
    fn drop(&mut self) {
        info!("anvil forked node shutting down");
    }
}
