//! # Datadog MCP Server
//!
//! A Model Context Protocol (MCP) applet that integrates with the
//! **Datadog Metrics API** via `weil_rs::datadog::DatadogClient` to:
//! - parse human-friendly time ranges into Unix timestamps,
//! - query time-series metrics and emit a `Plottable` dataset for visualization,
//! - list available metric names in the Datadog account.
//!
//! ## Overview
//! - **Auth/Config**: `Secrets<DatadogConfig>` must provide `site`, `api_key`, `app_key`.
//!   Example sites include `datadoghq.com`, `datadoghq.eu`, etc.
//! - **Client**: Uses `DatadogClient` from `weil_rs::datadog` with `DatadogConfig::new(...)`.
//! - **Time Parsing**: `AI::parse_human_time` converts natural phrases (e.g. “last 15 minutes”)
//!   into `{ from, to }` Unix timestamp strings.
//! - **Plotting**: `query_metrics` returns a `Plottable` with labeled time-series.
//! - **MCP Surface**: `tools()` returns JSON tool specs for agentic calls;
//!   `prompts()` is a placeholder for future prompt templates.
//!
//! ## Supported Operations
//! - `get_from_to_unix_timestamp(human_time)` — parse human time → `{ from, to }` (Unix strings).
//! - `query_metrics(query_str, from, to)` — call Datadog Metrics Query API and build `Plottable`.
//! - `list_metrics()` — list metric IDs available in the account.
//!
//! ## Notes
//! - This update adds **documentation only**; there are **no functional code changes**.
//! - Ensure `Secrets<DatadogConfig>` is populated at deploy time.

use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::{
    collections::plottable::Plottable, config::Secrets, datadog::DatadogClient, runtime::Runtime,
};

/// Datadog API credentials and site configuration.
///
/// - `site`: Datadog site domain (e.g., `"datadoghq.com"`, `"datadoghq.eu"`).
/// - `api_key`: Datadog API key.
/// - `app_key`: Datadog application key.
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct DatadogConfig {
    site: String,
    api_key: String,
    app_key: String,
}

/// A simple inclusive time window expressed as Unix timestamp strings.
#[derive(Debug, Serialize, Deserialize)]
pub struct FromToInterval {
    /// Start time as Unix timestamp string.
    from: String,
    /// End time as Unix timestamp string.
    to: String,
}

/// Public MCP trait for Datadog operations.
///
/// Each method propagates errors as `String` for simple agent consumption.
trait Datadog {
    /// Construct a new contract state with empty `Secrets<DatadogConfig>`.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Parse a human-friendly time phrase into `{ from, to }` Unix timestamps.
    ///
    /// Example inputs: `"right now"`, `"last 15 minutes"`, `"6 hours"`, `"1 day"`, `"one month"`.
    /// Uses `Runtime::parse_human_time` under the hood.
    async fn get_from_to_unix_timestamp(
        &self,
        human_time: String,
    ) -> Result<FromToInterval, String>;

    /// Query Datadog Metrics API for a time-series and return a `Plottable`.
    ///
    /// - `query_str`: Datadog metric query (e.g., `avg:system.cpu.user{*}`).
    /// - `from`, `to`: Unix timestamp strings (will be parsed to `i64`).
    ///
    /// Builds a labeled time-series `Plottable` where each Datadog series becomes a line,
    /// labeled with aggregation and display name; points come from `series.pointlist`.
    async fn query_metrics(
        &self,
        query_str: String,
        from: String,
        to: String,
    ) -> Result<Plottable, String>;

    /// Return a list of metric IDs available in the Datadog account.
    async fn list_metrics(&self) -> Result<Vec<String>, String>;

    /// JSON schema describing callable MCP tools.
    fn tools(&self) -> String;

    /// Placeholder for agent prompt templates.
    fn prompts(&self) -> String;
}

/// Contract state holding Datadog credentials in secret storage.
///
/// `secrets` must contain a valid `DatadogConfig` at runtime for all operations.
#[derive(Serialize, Deserialize, WeilType)]
pub struct DatadogContractState {
    // define your contract state here!
    /// Secure wrapper containing `DatadogConfig` (site, api_key, app_key).
    secrets: Secrets<DatadogConfig>,
}

#[smart_contract]
impl Datadog for DatadogContractState {
    /// Initialize an empty contract state with a new `Secrets<DatadogConfig>` container.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(DatadogContractState {
            secrets: Secrets::new(),
        })
    }

    /// Convert a human-readable time phrase to `{ from, to }` Unix timestamp strings.
    ///
    /// Delegates to `Runtime::parse_human_time`, returning the parsed interval verbatim.
    #[query]
    async fn get_from_to_unix_timestamp(
        &self,
        human_time: String,
    ) -> Result<FromToInterval, String> {
        let interval = Runtime::parse_human_time(&human_time).map_err(|err| err.to_string())?;

        Ok(FromToInterval {
            from: interval.from,
            to: interval.to,
        })
    }

    /// Query Datadog metrics and produce a time-series `Plottable` for visualization.
    ///
    /// - Parses `from`/`to` into `i64` Unix timestamps.
    /// - Builds a default time-series plot with axis labels and a series per Datadog timeseries.
    /// - Fails if `result.series` is empty.
    #[query(plottable)]
    async fn query_metrics(
        &self,
        query_str: String,
        from: String,
        to: String,
    ) -> Result<Plottable, String> {
        let config = weil_rs::datadog::DatadogConfig::new(
            self.secrets.config().site,
            self.secrets.config().api_key,
            self.secrets.config().app_key,
        );

        let client = DatadogClient::new(config);

        let parsed_from = from.parse::<i64>().map_err(|err| err.to_string())?;
        let parsed_to = to.parse::<i64>().map_err(|err| err.to_string())?;

        let mut plot = Plottable::new_with_time_series()
            .label(format!("plot for: {}", query_str))
            .x_axis_label("timestamp".to_string())
            .y_axis_label("metric".to_string());

        let result = client
            .query(query_str, parsed_from, parsed_to)
            .map_err(|err| err.to_string())?;

        if result.series.len() == 0 {
            return Err("no series data returned in response from Datadog".to_string());
        }

        for series in result.series {
            // Each series contributes a labeled line with its point list.
            plot.add_series(
                format!("{} {}", series.aggr, series.display_name),
                series.pointlist,
            );
        }

        Ok(plot)
    }

    /// List metric IDs available in the configured Datadog account.
    ///
    /// Returns a `Vec<String>` of `metrics.data[i].id`.
    #[query]
    async fn list_metrics(&self) -> Result<Vec<String>, String> {
        let config = weil_rs::datadog::DatadogConfig::new(
            self.secrets.config().site,
            self.secrets.config().api_key,
            self.secrets.config().app_key,
        );

        let client = DatadogClient::new(config);
        let metrics = client.list_metrics().map_err(|err| err.to_string())?;

        Ok(metrics.data.iter().map(|x| x.id.to_string()).collect())
    }

    /// Machine-readable MCP tool specifications for `get_from_to_unix_timestamp`,
    /// `query_metrics`, and `list_metrics`.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "get_from_to_unix_timestamp",
      "description": "Gets the current unix timestamp from the human time string which might be substring of a larger user prompt.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "human_time": {
            "type": "string",
            "description": "human time for eg. 'right now', '15 minutes', '6 hours', '1 day', 'one month'\n"
          }
        },
        "required": [
          "human_time"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "query_metrics",
      "description": "Retrieve metrics from Datadog using the Metrics Query API.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "Datadog metric query, e.g. 'avg:system.cpu.user{*}'\n"
          },
          "from": {
            "type": "string",
            "description": "Start time in unix timestamp.\n"
          },
          "to": {
            "type": "string",
            "description": "End time in unix timstamp.\n"
          }
        },
        "required": [
          "query_str",
          "from",
          "to"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_metrics",
      "description": "List all the Datadog metrics.\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  }
]"#.to_string()
    }

    /// Placeholder for prompt templates. Currently returns an empty `prompts` set.
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}
