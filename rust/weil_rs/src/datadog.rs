//! # Datadog HTTP Client (minimal wrapper)
//!
//! This module provides a lightweight client around a subset of the
//! [Datadog HTTP APIs](https://docs.datadoghq.com/api/latest/metrics/):
//!
//! - List available metrics (`/api/v2/metrics`)
//! - Query time-series metrics via the Datadog query DSL (`/api/v1/query`)
//!
//! Authentication is performed with `DD-API-KEY` and `DD-APPLICATION-KEY` headers.
//! The `site` (e.g., `api.datadoghq.com`, `api.us5.datadoghq.com`) selects the region.
//!
//! ## Usage
//! ```rust
//! # use your_crate::{DatadogClient, DatadogConfig};
//! let cfg = DatadogConfig::new(
//!     "api.datadoghq.com".to_string(),
//!     "<api_key>".to_string(),
//!     "<app_key>".to_string(),
//! );
//! let dd = DatadogClient::new(cfg);
//! // List metrics
//! // let metrics = dd.list_metrics()?;
//! // Query timeseries
//! // let res = dd.query("avg:system.cpu.user{*}".to_string(), 1_700_000_000, 1_700_003_600)?;
//! ```

use crate::http::{HttpClient, HttpMethod};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration required to authenticate and route Datadog API requests.
///
/// - `site`: Datadog site/region host, e.g. `"api.datadoghq.com"` or `"api.us5.datadoghq.com"`
/// - `api_key`: Datadog API key (sent as `DD-API-KEY`)
/// - `app_key`: Datadog Application key (sent as `DD-APPLICATION-KEY`)
#[derive(Serialize, Deserialize)]
pub struct DatadogConfig {
    site: String,
    api_key: String,
    app_key: String,
}

impl DatadogConfig {
    /// Create a new [`DatadogConfig`].
    ///
    /// # Arguments
    /// * `site` — Datadog site hostname (no scheme), e.g. `"api.datadoghq.com"`
    /// * `api_key` — Datadog API key
    /// * `app_key` — Datadog Application key
    pub fn new(site: String, api_key: String, app_key: String) -> Self {
        DatadogConfig {
            site,
            api_key,
            app_key,
        }
    }
}

/// Client which wraps a subset of Datadog's Metrics APIs.
///
/// See: <https://docs.datadoghq.com/api/latest/metrics/>
#[derive(Serialize, Deserialize)]
pub struct DatadogClient {
    /// Authentication and site selection.
    config: DatadogConfig,
}

impl DatadogClient {
    /// Create a new [`DatadogClient`].
    pub fn new(config: DatadogConfig) -> Self {
        DatadogClient { config }
    }

    /// Retrieve the list of available metric **definitions** from Datadog.
    ///
    /// Makes a `GET https://{site}/api/v2/metrics` request, attaching
    /// `DD-API-KEY` and `DD-APPLICATION-KEY` headers.
    ///
    /// # Returns
    /// A parsed [`ListMetricsResponse`] containing metric metadata.
    ///
    /// # Errors
    /// Propagates HTTP/transport errors from [`HttpClient::send`] and JSON decoding
    /// errors from [`Response::json`](crate::http::Response::json).
    pub fn list_metrics(&self) -> Result<ListMetricsResponse, anyhow::Error> {
        let mut req = HttpClient::request(
            &format!("https://{}/api/v2/metrics", self.config.site),
            HttpMethod::Get,
        );

        let mut headers: HashMap<String, String> = HashMap::default();

        headers.insert("DD-API-KEY".to_string(), self.config.api_key.to_string());
        headers.insert(
            "DD-APPLICATION-KEY".to_string(),
            self.config.app_key.to_string(),
        );

        req = req.headers(headers);

        let resp = req.send()?;
        let parsed_resp = resp.json::<ListMetricsResponse>()?;

        Ok(parsed_resp)
    }

    /// Query Datadog time-series metrics using the Datadog query DSL.
    ///
    /// Makes a `GET https://{site}/api/v1/query` request with `from`, `to`, and `query`
    /// parameters. Timestamps are **seconds since epoch**.
    ///
    /// # Arguments
    /// * `query_str` — Datadog query string (e.g., `"avg:system.cpu.user{*}"`)
    /// * `from` — start timestamp (epoch seconds)
    /// * `to` — end timestamp (epoch seconds)
    ///
    /// # Returns
    /// A parsed [`QueryResponse`] including one or more series with point lists.
    ///
    /// # Errors
    /// Propagates HTTP/transport errors from [`HttpClient::send`] and JSON decoding
    /// errors from [`Response::json`](crate::http::Response::json).
    pub fn query(
        &self,
        query_str: String,
        from: i64,
        to: i64,
    ) -> Result<QueryResponse, anyhow::Error> {
        let mut req = HttpClient::request(
            &format!(
                "https://{}/api/v1/query?from={}&to={}&query={}",
                self.config.site, from, to, query_str
            ),
            HttpMethod::Get,
        );

        let mut headers: HashMap<String, String> = HashMap::default();

        headers.insert("DD-API-KEY".to_string(), self.config.api_key.to_string());
        headers.insert(
            "DD-APPLICATION-KEY".to_string(),
            self.config.app_key.to_string(),
        );

        req = req.headers(headers);

        let resp = req.send()?;
        let parsed_resp = resp.json::<QueryResponse>()?;

        Ok(parsed_resp)
    }
}

/// Minimal metric definition returned by `/api/v2/metrics`.
#[derive(Serialize, Deserialize)]
pub struct MetricInfo {
    /// Metric type (as returned by Datadog, e.g., `"gauge"`, `"count"`).
    #[serde(rename = "type")]
    pub ty: String,
    /// Metric identifier/name.
    pub id: String,
}

/// Response body for `GET /api/v2/metrics`.
#[derive(Serialize, Deserialize)]
pub struct ListMetricsResponse {
    /// Collection of metric definitions.
    pub data: Vec<MetricInfo>,
}

/// A single queried series entry in the time-series response.
///
/// Fields follow Datadog's `/api/v1/query` response schema.
#[derive(Serialize, Deserialize)]
pub struct QuerySeriesEntry {
    /// Aggregation used (e.g., `"avg"`, `"sum"`).
    pub aggr: String,
    /// Metric name.
    pub metric: String,
    /// Raw expression for this series (if part of a compound query).
    pub expression: String,
    /// Rollup interval (seconds).
    pub interval: u32,
    /// Number of points in the series.
    pub length: u32,
    /// Series start time (epoch seconds).
    pub start: i64,
    /// Series end time (epoch seconds).
    pub end: i64,
    /// Scope (tag filter) applied to the series.
    pub scope: String,
    /// List of `(timestamp, value)` points. Timestamps are epoch seconds.
    pub pointlist: Vec<(f32, f32)>,
    /// Readable display name.
    pub display_name: String,
}

/// Response body for `GET /api/v1/query`.
#[derive(Serialize, Deserialize)]
pub struct QueryResponse {
    /// Request status (e.g., `"ok"`).
    pub status: String,
    /// Response type descriptor.
    pub res_type: String,
    /// API response schema version.
    pub resp_version: u32,
    /// The query string that was executed.
    pub query: String,
    /// Start time (epoch seconds) used by Datadog.
    pub from_date: i64,
    /// End time (epoch seconds) used by Datadog.
    pub to_date: i64,
    /// Returned series.
    pub series: Vec<QuerySeriesEntry>,
}
