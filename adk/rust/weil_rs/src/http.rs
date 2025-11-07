use crate::idempod::{request::OutcallRequest, response::OutcallResponse};
use crate::runtime::{get_length_prefixed_bytes_from_result, read_bytes_from_memory};
use rustc_hash::FxHashMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

#[link(wasm_import_module = "env")]
extern "C" {
    fn make_http_outcall(ptr: i32) -> i32;
}

/// Available HTTP methods
#[derive(Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
}

impl ToString for HttpMethod {
    fn to_string(&self) -> String {
        match self {
            HttpMethod::Get => "GET".to_string(),
            HttpMethod::Post => "POST".to_string(),
            HttpMethod::Put => "PUT".to_string(),
            HttpMethod::Delete => "DELETE".to_string(),
            HttpMethod::Patch => "PATCH".to_string(),
            HttpMethod::Head => "HEAD".to_string(),
        }
    }
}

/// HTTP client for making outcalls from inside of the WeilApplet.
#[derive(Clone, Copy)]
pub struct HttpClient;

impl HttpClient {
    pub fn new() -> Self {
        HttpClient
    }

    /// Constructs the `RequestBuilder` with the provided method and url.
    pub fn request(url: &str, method: HttpMethod) -> RequestBuilder {
        RequestBuilder {
            url: url.to_string(),
            method,
            headers: FxHashMap::default(),
            body: None,
            query_params: Vec::default(),
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// A builder to construct the properties of a Request.
pub struct RequestBuilder {
    url: String,
    method: HttpMethod,
    headers: FxHashMap<String, String>,
    body: Option<String>,
    query_params: Vec<(String, String)>,
}

impl RequestBuilder {
    /// Add a set of Headers to the existing ones on this `RequestBuilder`.
    /// The headers will be merged in to any already set.
    pub fn headers(mut self, headers: HashMap<String, String>) -> RequestBuilder {
        let existing_headers = &mut self.headers;

        for (header_name, header_value) in headers {
            existing_headers.insert(header_name, header_value);
        }

        self
    }

    /// Modifies the URL of this request, adding the parameters provided.
    pub fn query(mut self, query_params: Vec<(String, String)>) -> RequestBuilder {
        for entry in query_params {
            self.query_params.push(entry);
        }

        self
    }

    /// Set the request body as raw text.
    pub fn body(mut self, body: String) -> RequestBuilder {
        self.body = Some(body);

        self
    }

    pub fn json<T>(mut self, json: &T) -> RequestBuilder
    where
        T: Serialize + ?Sized,
    {
        let serialized_body = serde_json::to_string(json).unwrap();
        self.body = Some(serialized_body);

        self.headers
            .insert("Content-Type".to_string(), "application/json".to_string());

        self
    }

    /// Set the request body as form data.
    /// This will URL-encode the provided key-value pairs and set Content-Type to application/x-www-form-urlencoded.
    pub fn form(mut self, form_data: HashMap<String, String>) -> RequestBuilder {
        let encoded_data = form_data
            .iter()
            .map(|(key, value)| format!("{}={}", Self::url_encode(key), Self::url_encode(value)))
            .collect::<Vec<_>>()
            .join("&");

        self.body = Some(encoded_data);

        self.headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );

        self
    }

    /// URL encode a string according to RFC 3986.
    /// This is used internally by the form() method.
    fn url_encode(input: &str) -> String {
        input
            .chars()
            .map(|c| match c {
                // Unreserved characters (don't need encoding)
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                // Space becomes +
                ' ' => "+".to_string(),
                // Everything else gets percent-encoded
                _ => {
                    let mut buf = [0; 4];
                    let encoded_char = c.encode_utf8(&mut buf);
                    encoded_char
                        .bytes()
                        .map(|b| format!("%{:02X}", b))
                        .collect()
                }
            })
            .collect()
    }

    /// Sends it to the target URL, returning a `HttpResponse`.
    pub fn send(self) -> Result<HttpResponse, anyhow::Error> {
        let args = OutcallRequest {
            url: self.url,
            method: self.method.to_string(),
            headers: self.headers,
            body: self.body,
            query_params: self.query_params,
        };

        let args_buf = get_length_prefixed_bytes_from_result(Ok(args));
        let result_ptr = unsafe { make_http_outcall(args_buf.as_ptr() as i32) };
        let serialized_result = read_bytes_from_memory(result_ptr)?;
        let response: OutcallResponse = serde_json::from_str(&serialized_result)?;
        Ok(HttpResponse {
            status: response.status,
            body: response.body,
        })
    }
}

/// Response from the HTTP outcall request.
/// This is returned from the `send` method on `RequestBuilder`.
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpResponse {
    status: u16,
    body: String,
}

impl HttpResponse {
    /// Get the HTTP status code of the response.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Get the full response text.
    pub fn text(self) -> String {
        self.body
    }

    /// Try to deserialize the response body as JSON.
    pub fn json<T: DeserializeOwned>(self) -> Result<T, anyhow::Error> {
        let resp = serde_json::from_str::<T>(&self.body)?;

        Ok(resp)
    }
}
