use std::any::TypeId;

use headers::ContentType;
use http::StatusCode;
use tokio::sync::{mpsc, oneshot};
use utoipa::openapi::{RefOr, Schema};

use super::collectors::Collectors;
use super::operation::CalledOperation;
use super::schema::{SchemaEntry, Schemas};

/// Channel buffer size for collector messages.
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Messages sent to the collector task for schema and operation collection.
pub(in crate::client) enum CollectorMessage {
    /// Add schemas from path/query/header parameters.
    AddSchemas(Schemas),

    /// Add a single schema entry (e.g., from request body).
    AddSchemaEntry(SchemaEntry),

    /// Add an example to an existing schema by TypeId.
    AddExample {
        type_id: TypeId,
        type_name: &'static str,
        example: serde_json::Value,
    },

    /// Register a complete operation after HTTP call.
    RegisterOperation(CalledOperation),

    /// Register a response for an operation.
    RegisterResponse {
        operation_id: String,
        status: StatusCode,
        content_type: Option<ContentType>,
        schema: Option<RefOr<Schema>>,
        description: String,
    },

    /// Register a response with an example value.
    RegisterResponseWithExample {
        operation_id: String,
        status: StatusCode,
        content_type: Option<ContentType>,
        schema: RefOr<Schema>,
        example: serde_json::Value,
    },

    /// Request to retrieve final Collectors for OpenAPI generation.
    GetCollectors(oneshot::Sender<Collectors>),
}

/// Sender for collector messages, encapsulating the channel implementation.
#[derive(Debug, Clone)]
pub(in crate::client) struct CollectorSender {
    inner: mpsc::Sender<CollectorMessage>,
}

impl CollectorSender {
    /// Creates a dummy sender for skip_collection cases.
    pub(in crate::client) fn dummy() -> Self {
        let (sender, _) = mpsc::channel::<CollectorMessage>(1);
        Self { inner: sender }
    }

    /// Sends a message to the collector task.
    pub(in crate::client) async fn send(&self, msg: CollectorMessage) {
        self.inner
            .send(msg)
            .await
            .expect("Collector task should be running");
    }
}

/// Handle for sending messages to the collector task.
#[derive(Debug, Clone)]
pub(in crate::client) struct CollectorHandle {
    sender: CollectorSender,
}

impl CollectorHandle {
    /// Creates a new collector task and returns a handle.
    pub(in crate::client) fn spawn() -> Self {
        let (sender, receiver) = mpsc::channel::<CollectorMessage>(CHANNEL_BUFFER_SIZE);

        tokio::spawn(collector_task(receiver));

        Self {
            sender: CollectorSender { inner: sender },
        }
    }

    /// Returns a clone of the sender for passing to ApiCall/CallResult.
    pub(in crate::client) fn sender(&self) -> CollectorSender {
        self.sender.clone()
    }

    /// Request final collectors for OpenAPI generation.
    pub(in crate::client) async fn get_collectors(&self) -> Collectors {
        let (tx, rx) = oneshot::channel();
        self.sender.send(CollectorMessage::GetCollectors(tx)).await;
        rx.await.expect("Collector task should respond")
    }
}

/// Background task that receives collector messages and updates the Collectors.
async fn collector_task(mut receiver: mpsc::Receiver<CollectorMessage>) {
    let mut collectors = Collectors::default();

    while let Some(msg) = receiver.recv().await {
        match msg {
            CollectorMessage::AddSchemas(schemas) => {
                collectors.collect_schemas(schemas);
            }
            CollectorMessage::AddSchemaEntry(entry) => {
                collectors.collect_schema_entry(entry);
            }
            CollectorMessage::AddExample {
                type_id,
                type_name,
                example,
            } => {
                collectors
                    .schemas
                    .add_example_by_id(type_id, type_name, example);
            }
            CollectorMessage::RegisterOperation(operation) => {
                collectors.collect_operation(operation);
            }
            CollectorMessage::RegisterResponse {
                operation_id,
                status,
                content_type,
                schema,
                description,
            } => {
                collectors.register_response(
                    &operation_id,
                    status,
                    content_type.as_ref(),
                    schema,
                    description,
                );
            }
            CollectorMessage::RegisterResponseWithExample {
                operation_id,
                status,
                content_type,
                schema,
                example,
            } => {
                collectors.register_response_with_example(
                    &operation_id,
                    status,
                    content_type.as_ref(),
                    schema,
                    example,
                );
            }
            CollectorMessage::GetCollectors(responder) => {
                let _ = responder.send(collectors.clone());
            }
        }
    }
}
