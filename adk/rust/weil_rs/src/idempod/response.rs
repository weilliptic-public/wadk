use serde::{Deserialize, Serialize};

/// The result of executing an outcall (HTTP-style response).
///
/// - `status`: HTTP status code (e.g., 200, 404).
/// - `body`: UTF-8 response body. If the server returns non-UTF-8 bytes,
///   consider switching to `Vec<u8>` (see variant B).
///
/// # Example
/// ```
/// use your_crate::OutcallResponse;
///
/// let resp = OutcallResponse { status: 200, body: "ok".into() };
/// assert!(resp.is_success());
/// assert_eq!(resp.text(), "ok");
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OutcallResponse {
    /// HTTP status code (0–599 are typical).
    pub status: u16,

    /// UTF-8 response body.
    pub body: String,
}

impl OutcallResponse {
    /// Returns true if `status` is in [200, 299].
    pub fn is_success(&self) -> bool {
        (200..=299).contains(&self.status)
    }

    /// Returns the body as `&str`.
    pub fn text(&self) -> &str {
        &self.body
    }

    /// Attempt to parse the body as JSON into `T`.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.body)
    }
}
