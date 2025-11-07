use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use urlencoding::{decode, encode};
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::collections::plottable::Plottable;
use weil_rs::config::Secrets;
use weil_rs::http::{HttpClient, HttpMethod};

/// Gets the current timestamp in milliseconds from the timestamp API.
/// Returns the timestamp as a u64 value.
async fn get_current_timestamp() -> Result<u64, String> {
    let url = "https://aisenseapi.com/services/v1/timestamp";

    let response = HttpClient::request(url, HttpMethod::Get)
        .send()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let response_text = response.text();

    // Parse the JSON response to extract timestamp
    use serde_json::Value;
    let json: Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse timestamp response: {}", e))?;

    let timestamp_seconds = json["timestamp"]
        .as_u64()
        .ok_or("Invalid timestamp format in response")?;

    // Convert seconds to milliseconds
    Ok(timestamp_seconds * 1000)
}

/// Calculates from and to timestamps for recent data methods using HTTP API.
/// Returns (from_timestamp, to_timestamp) as u64 values in milliseconds.
async fn calculate_recent_timestamps(seconds_back: u64) -> Result<(u64, u64), String> {
    let current_timestamp = get_current_timestamp().await?;
    let from_timestamp = current_timestamp - (seconds_back * 1000);
    Ok((from_timestamp, current_timestamp))
}

/// Normalizes timestamp input to milliseconds.
/// Handles timestamps in either seconds or milliseconds format.
fn normalize_timestamp(ts_str: &str) -> Option<i64> {
    if let Ok(ts) = ts_str.parse::<i64>() {
        // Year 2100 in seconds is approximately 4102444800
        // If the timestamp is in this range, it's likely in seconds
        if ts <= 4102444800 {
            // Likely seconds, convert to milliseconds
            Some(ts * 1000)
        } else {
            // Likely already in milliseconds
            Some(ts)
        }
    } else {
        None
    }
}

/// Configuration for Alpha Vantage API access.
#[derive(Debug, Serialize, Deserialize, WeilType, Default, Clone)]
pub struct StocksConfig {
    /// Alpha Vantage API key for authentication
    api_key_1: String,
    api_key_2: String,
    api_key_3: String,
    api_key_4: String,
    api_key_5: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FromToInterval {
    from: String,
    to: String,
}

/// Represents a single data point in a time series.
#[derive(Debug, Serialize, Deserialize, WeilType, Clone)]
pub struct StockDataPoint {
    /// Timestamp of the data point
    timestamp: String,
    /// Opening price
    open: String,
    /// Highest price
    high: String,
    /// Lowest price
    low: String,
    /// Closing price
    close: String,
    /// Trading volume
    volume: String,
}

/// Represents time series data for a stock.
#[derive(Debug, Serialize, Deserialize, WeilType, Clone)]
pub struct TimeSeriesData {
    /// Stock symbol
    symbol: String,
    /// Time interval (e.g., "5min", "daily")
    interval: String,
    /// List of data points
    data_points: Vec<StockDataPoint>,
}

/// Represents real-time quote data for a stock.
#[derive(Debug, Serialize, Deserialize, WeilType, Clone)]
pub struct QuoteData {
    /// Stock symbol
    symbol: String,
    /// Current price
    price: String,
    /// Price change
    change: String,
    /// Price change percentage
    change_percent: String,
    /// Trading volume
    volume: String,
    /// Latest trading day
    latest_trading_day: String,
}

/// Trait defining Alpha Vantage stock data operations.
trait Stocks {
    /// Creates a new Alpha Vantage stocks contract instance.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    fn get_api_key(&self) -> String;

    /// Get intraday time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    async fn get_intraday_data_range(
        &self,
        symbol: String,
        interval: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String>;

    /// Get intraday time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    async fn get_intraday_data_recent(
        &self,
        symbol: String,
        interval: String,
        seconds_back: String,
    ) -> Result<Plottable, String>;

    /// Get daily time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    async fn get_daily_data_range(
        &self,
        symbol: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String>;

    /// Get daily time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    async fn get_daily_data_recent(
        &self,
        symbol: String,
        seconds_back: String,
    ) -> Result<Plottable, String>;

    /// Get weekly time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    async fn get_weekly_data_range(
        &self,
        symbol: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String>;

    /// Get weekly time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    async fn get_weekly_data_recent(
        &self,
        symbol: String,
        seconds_back: String,
    ) -> Result<Plottable, String>;

    /// Get monthly time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    async fn get_monthly_data_range(
        &self,
        symbol: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String>;

    /// Get monthly time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    async fn get_monthly_data_recent(
        &self,
        symbol: String,
        seconds_back: String,
    ) -> Result<Plottable, String>;

    /// Get real-time quote for a stock.
    async fn get_quote(&self, symbol: String) -> Result<String, String>;

    /// Search for stocks by keyword.
    async fn search_symbol(&self, keywords: String) -> Result<String, String>;

    /// Get company overview and fundamental data.
    async fn get_company_overview(&self, symbol: String) -> Result<String, String>;

    /// Get earnings data for a company.
    async fn get_earnings(&self, symbol: String) -> Result<String, String>;

    /// Get Simple Moving Average (SMA) technical indicator.
    async fn get_sma(
        &self,
        symbol: String,
        interval: String,
        time_period: u32,
        series_type: String,
    ) -> Result<Plottable, String>;

    /// Get Relative Strength Index (RSI) technical indicator.
    async fn get_rsi(
        &self,
        symbol: String,
        interval: String,
        time_period: u32,
        series_type: String,
    ) -> Result<Plottable, String>;

    /// Returns the JSON schema defining available tools.
    fn tools(&self) -> String;

    /// Returns the JSON schema defining available prompts.
    fn prompts(&self) -> String;
}

/// Contract state for Alpha Vantage stock operations.
#[derive(Serialize, Deserialize, WeilType)]
pub struct StocksContractState {
    /// Alpha Vantage API configuration secrets
    secrets: Secrets<StocksConfig>,
}

#[smart_contract]
impl Stocks for StocksContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(StocksContractState {
            secrets: Secrets::new(),
        })
    }

    fn get_api_key(&self) -> String {
        self.secrets.config().api_key_2.clone()
    }

    /// Get intraday time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    #[query(plottable)]
    async fn get_intraday_data_range(
        &self,
        symbol: String,
        interval: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_INTRADAY&symbol={}&interval={}&outputsize={}&apikey={}",
                symbol, interval, "full", api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        let from_ts = from_timestamp.parse::<u64>().unwrap_or(0);
        let to_ts = to_timestamp.parse::<u64>().unwrap_or(u64::MAX);
        self.parse_multiple_symbols_to_plottable(symbol_responses, &interval, from_ts, to_ts)
    }

    /// Get intraday time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    #[query(plottable)]
    async fn get_intraday_data_recent(
        &self,
        symbol: String,
        interval: String,
        seconds_back: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_INTRADAY&symbol={}&interval={}&outputsize={}&apikey={}",
                symbol, interval, "full", api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        // Calculate the from timestamp based on seconds_back from now
        let seconds_back_u64 = seconds_back.parse::<u64>().unwrap_or(2592000); // Default to ~30 days
        let (from_timestamp, to_timestamp) = calculate_recent_timestamps(seconds_back_u64).await?;

        self.parse_multiple_symbols_to_plottable(
            symbol_responses,
            &interval,
            from_timestamp,
            to_timestamp,
        )
    }

    /// Get daily time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    #[query(plottable)]
    async fn get_daily_data_range(
        &self,
        symbol: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();

        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY&symbol={}&outputsize={}&apikey={}",
                symbol, "full", api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        let from_ts = from_timestamp.parse::<u64>().unwrap_or(0);
        let to_ts = to_timestamp.parse::<u64>().unwrap_or(u64::MAX);
        self.parse_multiple_symbols_to_plottable(symbol_responses, "daily", from_ts, to_ts)
    }

    /// Get daily time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    #[query(plottable)]
    async fn get_daily_data_recent(
        &self,
        symbol: String,
        seconds_back: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY&symbol={}&outputsize={}&apikey={}",
                symbol, "full", api_key
            );

            let response = self.make_api_request(&url).await?;

            symbol_responses.insert(symbol, response);
        }

        // Calculate the from timestamp based on seconds_back from now
        let seconds_back_u64 = seconds_back.parse::<u64>().unwrap_or(2592000); // Default to ~30 days
        let (from_timestamp, to_timestamp) = calculate_recent_timestamps(seconds_back_u64).await?;

        self.parse_multiple_symbols_to_plottable(
            symbol_responses,
            "daily",
            from_timestamp,
            to_timestamp,
        )
    }

    /// Get weekly time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    #[query(plottable)]
    async fn get_weekly_data_range(
        &self,
        symbol: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_WEEKLY&symbol={}&apikey={}",
                symbol, api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        let from_ts = from_timestamp.parse::<u64>().unwrap_or(0);
        let to_ts = to_timestamp.parse::<u64>().unwrap_or(u64::MAX);
        self.parse_multiple_symbols_to_plottable(symbol_responses, "weekly", from_ts, to_ts)
    }

    /// Get weekly time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    #[query(plottable)]
    async fn get_weekly_data_recent(
        &self,
        symbol: String,
        seconds_back: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_WEEKLY&symbol={}&apikey={}",
                symbol, api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        // Calculate the from timestamp based on seconds_back from now
        let seconds_back_u64 = seconds_back.parse::<u64>().unwrap_or(2592000); // Default to ~30 days
        let (from_timestamp, to_timestamp) = calculate_recent_timestamps(seconds_back_u64).await?;

        self.parse_multiple_symbols_to_plottable(
            symbol_responses,
            "weekly",
            from_timestamp,
            to_timestamp,
        )
    }

    /// Get monthly time series data for stocks with absolute time range.
    /// Use this method when you have specific start and end dates/times (e.g., "for year 2023", "from January to March")
    #[query(plottable)]
    async fn get_monthly_data_range(
        &self,
        symbol: String,
        from_timestamp: String,
        to_timestamp: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_MONTHLY&symbol={}&apikey={}",
                symbol, api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        let from_ts = from_timestamp.parse::<u64>().unwrap_or(0);
        let to_ts = to_timestamp.parse::<u64>().unwrap_or(u64::MAX);
        self.parse_multiple_symbols_to_plottable(symbol_responses, "monthly", from_ts, to_ts)
    }

    /// Get monthly time series data for stocks with relative time range.
    /// Use this method when you want data relative to now (e.g., "for the last month", "past 30 days", "last week")
    #[query(plottable)]
    async fn get_monthly_data_recent(
        &self,
        symbol: String,
        seconds_back: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();
        let mut symbol_responses = HashMap::new();

        // Create symbols vector from comma-separated input, trimming whitespace
        let symbols: Vec<String> = symbol
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for symbol in symbols {
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_MONTHLY&symbol={}&apikey={}",
                symbol, api_key
            );

            let response = self.make_api_request(&url).await?;
            symbol_responses.insert(symbol, response);
        }

        // Calculate the from timestamp based on seconds_back from now
        let seconds_back_u64 = seconds_back.parse::<u64>().unwrap_or(2592000); // Default to ~30 days
        let (from_timestamp, to_timestamp) = calculate_recent_timestamps(seconds_back_u64).await?;

        self.parse_multiple_symbols_to_plottable(
            symbol_responses,
            "monthly",
            from_timestamp,
            to_timestamp,
        )
    }

    /// Get real-time quote for a stock.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Stock symbol (e.g., "IBM", "AAPL")
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - JSON response from Alpha Vantage API
    /// * `Err(String)` - Error message if request fails
    #[query]
    async fn get_quote(&self, symbol: String) -> Result<String, String> {
        let api_key = self.get_api_key();

        let url = format!(
            "https://www.alphavantage.co/query?function=GLOBAL_QUOTE&symbol={}&apikey={}",
            symbol, api_key
        );

        self.make_api_request(&url).await
    }

    /// Search for stocks by keyword.
    ///
    /// # Arguments
    ///
    /// * `keywords` - Keywords to search for (e.g., "microsoft", "MSFT")
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - JSON response from Alpha Vantage API
    /// * `Err(String)` - Error message if request fails
    #[query]
    async fn search_symbol(&self, keywords: String) -> Result<String, String> {
        let api_key = self.get_api_key();

        let url = format!(
            "https://www.alphavantage.co/query?function=SYMBOL_SEARCH&keywords={}&apikey={}",
            keywords, api_key
        );

        self.make_api_request(&url).await
    }

    /// Get company overview and fundamental data.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Stock symbol (e.g., "IBM", "AAPL")
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - JSON response from Alpha Vantage API
    /// * `Err(String)` - Error message if request fails
    #[query]
    async fn get_company_overview(&self, symbol: String) -> Result<String, String> {
        let api_key = self.get_api_key();

        let url = format!(
            "https://www.alphavantage.co/query?function=OVERVIEW&symbol={}&apikey={}",
            symbol, api_key
        );

        self.make_api_request(&url).await
    }

    /// Get earnings data for a company.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Stock symbol (e.g., "IBM", "AAPL")
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - JSON response from Alpha Vantage API
    /// * `Err(String)` - Error message if request fails
    #[query]
    async fn get_earnings(&self, symbol: String) -> Result<String, String> {
        let api_key = self.get_api_key();

        let url = format!(
            "https://www.alphavantage.co/query?function=EARNINGS&symbol={}&apikey={}",
            symbol, api_key
        );

        self.make_api_request(&url).await
    }

    /// Get Simple Moving Average (SMA) technical indicator.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Stock symbol (e.g., "IBM", "AAPL")
    /// * `interval` - Time interval: "1min", "5min", "15min", "30min", "60min", "daily", "weekly", "monthly"
    /// * `time_period` - Time period for SMA calculation (e.g., 20, 50, 200)
    /// * `series_type` - Price type: "close", "open", "high", "low"
    ///
    /// # Returns
    ///
    /// * `Ok(Plottable)` - Plottable object with SMA indicator data
    /// * `Err(String)` - Error message if request fails
    #[query(plottable)]
    async fn get_sma(
        &self,
        symbol: String,
        interval: String,
        time_period: u32,
        series_type: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();

        let url = format!(
            "https://www.alphavantage.co/query?function=SMA&symbol={}&interval={}&time_period={}&series_type={}&apikey={}",
            symbol, interval, time_period, series_type, api_key
        );

        let response = self.make_api_request(&url).await?;
        self.parse_indicator_to_plottable(&response, &symbol, &format!("SMA({})", time_period))
    }

    /// Get Relative Strength Index (RSI) technical indicator.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Stock symbol (e.g., "IBM", "AAPL")
    /// * `interval` - Time interval: "1min", "5min", "15min", "30min", "60min", "daily", "weekly", "monthly"
    /// * `time_period` - Time period for RSI calculation (typically 14)
    /// * `series_type` - Price type: "close", "open", "high", "low"
    ///
    /// # Returns
    ///
    /// * `Ok(Plottable)` - Plottable object with RSI indicator data
    /// * `Err(String)` - Error message if request fails
    #[query(plottable)]
    async fn get_rsi(
        &self,
        symbol: String,
        interval: String,
        time_period: u32,
        series_type: String,
    ) -> Result<Plottable, String> {
        let api_key = self.get_api_key();

        let url = format!(
            "https://www.alphavantage.co/query?function=RSI&symbol={}&interval={}&time_period={}&series_type={}&apikey={}",
            symbol, interval, time_period, series_type, api_key
        );

        let response = self.make_api_request(&url).await?;
        self.parse_indicator_to_plottable(&response, &symbol, &format!("RSI({})", time_period))
    }

    /// Returns the JSON schema defining available tools for the stocks contract.
    ///
    /// # Returns
    ///
    /// * `String` - JSON string containing tool definitions for all stock operations.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "get_intraday_data_range",
      "description": "Get intraday time series data for stocks with absolute time range and return as plottable chart. Use this method when you have specific start and end dates/times (e.g., 'for year 2023', 'from January to March')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "interval": {
            "type": "string",
            "description": "Time interval: 1min, 5min, 15min, 30min, 60min\n",
            "enum": ["1min", "5min", "15min", "30min", "60min"]
          },
          "from_timestamp": {
            "type": "string",
            "description": "Start timestamp in milliseconds (inclusive lower bound)\n"
          },
          "to_timestamp": {
            "type": "string",
            "description": "End timestamp in milliseconds (inclusive upper bound)\n"
          }
        },
        "required": ["symbol", "interval", "from_timestamp", "to_timestamp"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_intraday_data_recent",
      "description": "Get intraday time series data for stocks with relative time range and return as plottable chart. Use this method when you want data relative to now (e.g., 'for the last month', 'past 30 days', 'last week')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "interval": {
            "type": "string",
            "description": "Time interval: 1min, 5min, 15min, 30min, 60min\n",
            "enum": ["1min", "5min", "15min", "30min", "60min"]
          },
          "seconds_back": {
            "type": "string",
            "description": "Number of seconds back from now (e.g., \"2592000\" for ~30 days, \"604800\" for 1 week)\n"
          }
        },
        "required": ["symbol", "interval", "seconds_back"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_daily_data_range",
      "description": "Get daily time series data for stocks with absolute time range and return as plottable chart. Use this method when you have specific start and end dates/times (e.g., 'for year 2023', 'from January to March')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "from_timestamp": {
            "type": "string",
            "description": "Start timestamp in milliseconds (inclusive lower bound)\n"
          },
          "to_timestamp": {
            "type": "string",
            "description": "End timestamp in milliseconds (inclusive upper bound)\n"
          }
        },
        "required": ["symbol", "from_timestamp", "to_timestamp"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_daily_data_recent",
      "description": "Get daily time series data for stocks with relative time range and return as plottable chart. Use this method when you want data relative to now (e.g., 'for the last month', 'past 30 days', 'last week')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "seconds_back": {
            "type": "string",
            "description": "Number of seconds back from now (e.g., \"2592000\" for ~30 days, \"604800\" for 1 week)\n"
          }
        },
        "required": ["symbol", "seconds_back"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_weekly_data_range",
      "description": "Get weekly time series data for stocks with absolute time range and return as plottable chart. Use this method when you have specific start and end dates/times (e.g., 'for year 2023', 'from January to March')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "from_timestamp": {
            "type": "string",
            "description": "Start timestamp in milliseconds (inclusive lower bound)\n"
          },
          "to_timestamp": {
            "type": "string",
            "description": "End timestamp in milliseconds (inclusive upper bound)\n"
          }
        },
        "required": ["symbol", "from_timestamp", "to_timestamp"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_weekly_data_recent",
      "description": "Get weekly time series data for stocks with relative time range and return as plottable chart. Use this method when you want data relative to now (e.g., 'for the last month', 'past 30 days', 'last week')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "seconds_back": {
            "type": "string",
            "description": "Number of seconds back from now (e.g., \"2592000\" for ~30 days, \"604800\" for 1 week)\n"
          }
        },
        "required": ["symbol", "seconds_back"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_monthly_data_range",
      "description": "Get monthly time series data for stocks with absolute time range and return as plottable chart. Use this method when you have specific start and end dates/times (e.g., 'for year 2023', 'from January to March')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "from_timestamp": {
            "type": "string",
            "description": "Start timestamp in milliseconds (inclusive lower bound)\n"
          },
          "to_timestamp": {
            "type": "string",
            "description": "End timestamp in milliseconds (inclusive upper bound)\n"
          }
        },
        "required": ["symbol", "from_timestamp", "to_timestamp"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_monthly_data_recent",
      "description": "Get monthly time series data for stocks with relative time range and return as plottable chart. Use this method when you want data relative to now (e.g., 'for the last month', 'past 30 days', 'last week')\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbols as comma-separated string (e.g., \"IBM, AAPL, GOOGL\")\n"
          },
          "seconds_back": {
            "type": "string",
            "description": "Number of seconds back from now (e.g., \"2592000\" for ~30 days, \"604800\" for 1 week)\n"
          }
        },
        "required": ["symbol", "seconds_back"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_quote",
      "description": "Get real-time quote for a stock\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbol (e.g., IBM, AAPL)\n"
          }
        },
        "required": ["symbol"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "search_symbol",
      "description": "Search for stocks by keyword\n",
      "parameters": {
        "type": "object",
        "properties": {
          "keywords": {
            "type": "string",
            "description": "Keywords to search for (e.g., microsoft, MSFT)\n"
          }
        },
        "required": ["keywords"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_company_overview",
      "description": "Get company overview and fundamental data\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbol (e.g., IBM, AAPL)\n"
          }
        },
        "required": ["symbol"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_earnings",
      "description": "Get earnings data for a company\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbol (e.g., IBM, AAPL)\n"
          }
        },
        "required": ["symbol"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_sma",
      "description": "Get technical indicator data (SMA - Simple Moving Average) and return as plottable chart\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbol (e.g., IBM, AAPL)\n"
          },
          "interval": {
            "type": "string",
            "description": "Time interval: 1min, 5min, 15min, 30min, 60min, daily, weekly, monthly\n",
            "enum": ["1min", "5min", "15min", "30min", "60min", "daily", "weekly", "monthly"]
          },
          "time_period": {
            "type": "integer",
            "description": "Time period for SMA calculation (e.g., 20, 50, 200)\n"
          },
          "series_type": {
            "type": "string",
            "description": "Price type: close, open, high, low\n",
            "enum": ["close", "open", "high", "low"]
          }
        },
        "required": ["symbol", "interval", "time_period", "series_type"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_rsi",
      "description": "Get technical indicator data (RSI - Relative Strength Index) and return as plottable chart\n",
      "parameters": {
        "type": "object",
        "properties": {
          "symbol": {
            "type": "string",
            "description": "Stock symbol (e.g., IBM, AAPL)\n"
          },
          "interval": {
            "type": "string",
            "description": "Time interval: 1min, 5min, 15min, 30min, 60min, daily, weekly, monthly\n",
            "enum": ["1min", "5min", "15min", "30min", "60min", "daily", "weekly", "monthly"]
          },
          "time_period": {
            "type": "integer",
            "description": "Time period for RSI calculation (typically 14)\n"
          },
          "series_type": {
            "type": "string",
            "description": "Price type: close, open, high, low\n",
            "enum": ["close", "open", "high", "low"]
          }
        },
        "required": ["symbol", "interval", "time_period", "series_type"]
      }
    }
  }
]"#.to_string()
    }

    /// Returns the JSON schema defining available prompts for the stocks contract.
    ///
    /// # Returns
    ///
    /// * `String` - JSON string containing prompt definitions (currently empty).
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}

impl StocksContractState {
    /// Helper method to parse Alpha Vantage time series JSON and extract close price points with timestamp filters.
    ///
    /// # Arguments
    ///
    /// * `json_response` - The JSON response from Alpha Vantage API
    /// * `from` - Unix timestamp filter (inclusive lower bound) in milliseconds
    /// * `to` - Unix timestamp filter (inclusive upper bound) in milliseconds
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<(f32, f32)>)` - Vector of (timestamp, close_price) tuples
    /// * `Err(String)` - Error message if parsing fails
    fn parse_time_series_response_to_points(
        &self,
        json_response: &str,
        from: u64,
        to: u64,
    ) -> Result<Vec<(f32, f32)>, String> {
        use serde_json::Value;

        let json: Value = serde_json::from_str(json_response)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        // Find the time series data in the response
        let time_series_key = json
            .as_object()
            .ok_or("Invalid JSON structure")?
            .keys()
            .find(|key| key.contains("Time Series"))
            .ok_or("No time series data found in response")?;

        let time_series = json[time_series_key]
            .as_object()
            .ok_or("Time series data is not an object")?;

        let mut timestamps = Vec::new();
        let mut closes = Vec::new();

        for (date, data) in time_series {
            if let Some(data_obj) = data.as_object() {
                // Convert YYYY-MM-DD to unix timestamp at 00:00:00 UTC
                let ts_opt = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| {
                        Utc.with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0)
                            .single()
                    })
                    .map(|dt| dt.timestamp() * 1000);

                if let Some(ts) = ts_opt {
                    // Check timestamp filters
                    if ts < from as i64 {
                        continue;
                    }
                    if ts > to as i64 {
                        continue;
                    }

                    timestamps.push(ts as f32);
                    if let Some(close) = data_obj.get("4. close").and_then(|v| v.as_str()) {
                        closes.push(close.parse::<f32>().unwrap_or(0.0));
                    }
                }
            }
        }

        // Sort by timestamp (Alpha Vantage returns newest first, we want oldest first for plotting)
        let mut sorted_indices: Vec<usize> = (0..timestamps.len()).collect();
        sorted_indices.sort_by(|a, b| timestamps[*a].partial_cmp(&timestamps[*b]).unwrap());

        // Create data points for close prices
        let mut close_points = Vec::new();
        for &idx in &sorted_indices {
            let x = timestamps[idx];
            close_points.push((x, closes[idx]));
        }

        Ok(close_points)
    }

    /// Helper method to parse multiple symbol responses and convert to Plottable.
    ///
    /// # Arguments
    ///
    /// * `symbol_responses` - HashMap of symbol -> JSON response pairs
    /// * `interval` - Time interval for labeling
    /// * `from` - Unix timestamp filter (inclusive lower bound) in milliseconds
    /// * `to` - Unix timestamp filter (inclusive upper bound) in milliseconds
    ///
    /// # Returns
    ///
    /// * `Ok(Plottable)` - Plottable object with time series data for all symbols
    /// * `Err(String)` - Error message if parsing fails
    fn parse_multiple_symbols_to_plottable(
        &self,
        symbol_responses: HashMap<String, String>,
        interval: &str,
        from: u64,
        to: u64,
    ) -> Result<Plottable, String> {
        let mut plot = Plottable::new_with_time_series()
            .label(format!(
                "{} {} Price Data",
                symbol_responses
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                interval
            ))
            .x_axis_label("Date".to_string())
            .y_axis_label("Price".to_string());

        for (symbol, response) in symbol_responses {
            let close_points = self.parse_time_series_response_to_points(&response, from, to)?;
            plot.add_series(format!("{}", symbol), close_points);
        }

        Ok(plot)
    }

    /// Helper method to parse Alpha Vantage technical indicator JSON and convert to Plottable.
    ///
    /// # Arguments
    ///
    /// * `json_response` - The JSON response from Alpha Vantage API
    /// * `symbol` - Stock symbol for labeling
    /// * `indicator_name` - Name of the technical indicator
    ///
    /// # Returns
    ///
    /// * `Ok(Plottable)` - Plottable object with indicator data
    /// * `Err(String)` - Error message if parsing fails
    fn parse_indicator_to_plottable(
        &self,
        json_response: &str,
        symbol: &str,
        indicator_name: &str,
    ) -> Result<Plottable, String> {
        use serde_json::Value;

        let json: Value = serde_json::from_str(json_response)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        // Find the technical indicator data in the response
        let indicator_key = json
            .as_object()
            .ok_or("Invalid JSON structure")?
            .keys()
            .find(|key| key.contains("Technical Analysis"))
            .ok_or("No technical analysis data found in response")?;

        let technical_analysis = json[indicator_key]
            .as_object()
            .ok_or("Technical analysis data is not an object")?;

        let mut plot = Plottable::new_with_time_series()
            .label(format!("{} - {} Indicator", symbol, indicator_name))
            .x_axis_label("Date".to_string())
            .y_axis_label("Indicator Value".to_string());

        // Parse each data point and add to series
        let mut dates = Vec::new();
        let mut values = Vec::new();

        for (date, data) in technical_analysis {
            if let Some(data_obj) = data.as_object() {
                dates.push(date.clone());

                // Get the indicator value (usually under the indicator name key)
                if let Some(value) = data_obj.values().next().and_then(|v| v.as_str()) {
                    values.push(value.parse::<f32>().unwrap_or(0.0));
                }
            }
        }

        // Sort by date
        let mut sorted_indices: Vec<usize> = (0..dates.len()).collect();
        sorted_indices.sort_by(|a, b| dates[*a].cmp(&dates[*b]));

        // Create data points for the series
        let mut points = Vec::new();
        for (i, &idx) in sorted_indices.iter().enumerate() {
            let x = i as f32;
            points.push((x, values[idx]));
        }

        // Add series to plot
        plot.add_series(format!("{} {}", symbol, indicator_name), points);

        Ok(plot)
    }

    /// Helper method to make HTTP requests to Alpha Vantage API.
    ///
    /// # Arguments
    ///
    /// * `url` - The full URL to make the request to
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The response text from the API
    /// * `Err(String)` - Error message if the request fails
    async fn make_api_request(&self, url: &str) -> Result<String, String> {
        let response = HttpClient::request(url, HttpMethod::Get)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let response_text = response.text();

        // Check if the response contains an error message
        if response_text.contains("Error Message") || response_text.contains("Note:") {
            return Err(format!("Alpha Vantage API error: {}", response_text));
        }

        // Detect Alpha Vantage daily rate limit informational response and return standardized error
        if response_text.contains("\"Information\"")
            && response_text.contains("rate limit is 25 requests per day")
        {
            return Err(
                "Alpha Vantage API error: API rate limit of 25 requests per day is reached"
                    .to_string(),
            );
        }

        Ok(response_text)
    }
}
