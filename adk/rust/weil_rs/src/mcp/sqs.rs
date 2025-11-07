//! Types for interacting with a queue service (AWS SQS–compatible shape).
//!
//! These structs are `serde`-serializable so they can be sent over the wire
//! or persisted. They model common queue operations: list, create, delete,
//! send, receive, and batch-delete messages.
//!
//! ### Conventions
//! - `Credentials` may include a temporary `session_token`.
//! - Pagination is modeled with `next_token` where applicable.
//! - For batch APIs, success/failure are returned as string identifiers.
//!
//! ### Example
//! ```
//! use serde_json;
//! # use your_crate::*; // replace with the actual path
//!
//! let creds = Credentials {
//!     access_key_id: "AKIA...".into(),
//!     secret_access_key: "secret".into(),
//!     region: "us-west-2".into(),
//!     session_token: None,
//! };
//!
//! let params = SendMessagesParams {
//!     credentials: creds,
//!     queue: "my-queue".into(),
//!     messages: vec!["hello".into(), "world".into()],
//! };
//!
//! let json = serde_json::to_string(&params).unwrap();
//! assert!(json.contains("my-queue"));
//! ```

use serde::{Deserialize, Serialize};

/// Credentials used to authenticate against the queue service.
///
/// For AWS-style services:
/// - `access_key_id` / `secret_access_key` identify the principal.
/// - `region` selects the service region (e.g., `"us-east-1"`).
/// - `session_token` is present for temporary credentials (e.g., STS).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Credentials {
    /// Access key ID for the account/user/role.
    pub access_key_id: String,
    /// Secret key paired with `access_key_id`.
    pub secret_access_key: String,
    /// Service region (e.g., `"us-west-2"`).
    pub region: String,
    /// Optional session token for temporary creds.
    pub session_token: Option<String>,
}

/// Parameters for listing queues.
///
/// Supports prefix filtering, pagination token, and a soft limit via `max_results`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListQueuesParams {
    /// Authentication for the request.
    pub credentials: Credentials,
    /// Optional name prefix filter (e.g., `"prod-"`).
    pub prefix: Option<String>,
    /// Opaque pagination cursor returned from a previous call.
    pub next_token: Option<String>,
    /// Hint to limit number of results returned.
    pub max_results: Option<i32>,
}

/// Response from listing queues.
///
/// Contains queue names/URLs and an optional pagination token.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListQueuesResponse {
    /// List of queue identifiers (names or URLs, depending on implementation).
    pub queues: Vec<String>,
    /// Present when more results are available.
    pub next_token: Option<String>,
}

/// Parameters to create a queue with the given name.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateQueueParams {
    /// Authentication for the request.
    pub credentials: Credentials,
    /// Queue name to create.
    pub name: String,
}

/// Parameters to delete a queue with the given name.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteQueueParams {
    /// Authentication for the request.
    pub credentials: Credentials,
    /// Queue name to delete.
    pub name: String,
}

/// Parameters to send one or more messages to a queue.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SendMessagesParams {
    /// Authentication for the request.
    pub credentials: Credentials,
    /// Target queue (name or URL).
    pub queue: String,
    /// Message payloads to enqueue (UTF-8 strings).
    pub messages: Vec<String>,
}

/// Result of a batch send operation.
///
/// `failed` contains message identifiers that could not be enqueued.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SendMessagesResponse {
    /// Identifiers (or payloads) that failed to send.
    pub failed: Vec<String>,
}

/// Parameters to receive (pull) messages from a queue.
///
/// `max_results` requests up to N messages (service may return fewer).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReceiveMessagesParams {
    /// Authentication for the request.
    pub credentials: Credentials,
    /// Source queue (name or URL).
    pub queue: String,
    /// Maximum number of messages to retrieve.
    pub max_results: Option<i32>,
}

/// Messages received from a queue.
///
/// Each tuple is `(handle, body)` where `handle` is an opaque receipt
/// to acknowledge/delete the message later, and `body` is the message text.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReceiveMessagesResponse {
    /// Vector of `(receipt_handle, message_body)` pairs.
    pub received: Vec<(String, String)>,
}

/// Parameters to delete (ack) a batch of messages given their receipt handles.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteMessagesParams {
    /// Authentication for the request.
    pub credentials: Credentials,
    /// Target queue (name or URL).
    pub queue: String,
    /// Receipt handles to delete/acknowledge.
    pub handles: Vec<String>,
}

/// Result of a batch delete operation.
///
/// `successful` lists handles that were deleted; `failed` lists those that were not.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteMessagesResponse {
    /// Receipt handles successfully deleted.
    pub successful: Vec<String>,
    /// Receipt handles that failed to delete.
    pub failed: Vec<String>,
}
