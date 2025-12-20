#![allow(missing_docs)]

use rstest::fixture;
use tracing::info;

mod test_app;
pub use self::test_app::*;

mod client;
pub use self::client::*;

/// Result of listing observations with wildcard redaction.
#[derive(Debug)]
pub struct RedactedListResult {
    /// The actual list of observations (with real dynamic values).
    pub value: ListObservations,
    /// The redacted JSON (with stable values for documentation).
    pub redacted: serde_json::Value,
}

pub fn init_tracing() {
    // should be run once, fail otherwise, we skip that error
    let _ = tracing_subscriber::fmt()
        .pretty()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();

    info!("Tracing initialized");
}

#[fixture]
pub async fn app() -> TestApp {
    init_tracing();
    match TestApp::start().await {
        Ok(app) => app,
        Err(error) => {
            panic!("fail to start test app: {error:?}");
        }
    }
}
