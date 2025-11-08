use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// A single HTTP-style request your library can execute as an "outcall".
///
/// # Design
/// - `headers`: stored as `FxHashMap<String, String>` for fast lookups.
/// - `body`: optional to support GET/HEAD requests.
/// - `query_params`: serialized in the final URL in order.
/// - `method`: free-form string; typical values are `"GET"`, `"POST"`, etc.
///   Consider using `http::Method` if you want type safety.
///
/// # Invariants
/// - `url` should be an absolute URL (e.g., `https://api.example.com/v1`).
/// - Header keys are expected to be case-insensitive; callers should prefer lowercase.
///
/// # Example
/// Build and serialize a JSON POST with a query parameter:
/// ```
/// use rustc_hash::FxHashMap;
/// use serde_json::json;
/// use your_crate::OutcallRequest;
///
/// let mut headers = FxHashMap::default();
/// headers.insert("content-type".into(), "application/json".into());
///
/// let req = OutcallRequest {
///     url: "https://api.example.com/items".into(),
///     method: "POST".into(),
///     headers,
///     body: Some(json!({"name":"widget"}).to_string()),
///     query_params: vec![("region".into(), "us-west".into())],
/// };
///
/// // It serializes cleanly:
/// let as_json = serde_json::to_string(&req).unwrap();
/// assert!(as_json.contains("\"method\":\"POST\""));
/// ```
#[derive(Serialize, Deserialize, Debug)]
pub struct OutcallRequest {
    /// Absolute destination URL (scheme + host, optional path + query).
    pub url: String,

    /// HTTP method string (e.g., `"GET"`, `"POST"`, `"PUT"`).
    pub method: String,

    /// Request headers; header names should be lowercase.
    pub headers: FxHashMap<String, String>,

    /// Optional UTF-8 body. Use `None` for methods with no body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Query string to append to the URL, in the given order.
    pub query_params: Vec<(String, String)>,
}
