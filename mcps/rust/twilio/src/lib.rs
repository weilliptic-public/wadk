//! # Twilio Weilchain Applet
//!
//! A Model Context Protocol (MCP) server that integrates with the
//! **Twilio REST API (2010-04-01)** to send SMS messages, place voice calls
//! via TwiML, fetch recent messages, and retrieve account information.
//!
//! ## Overview
//! - **Auth**: HTTP Basic auth using `account_sid:auth_token` (Base64-encoded).
//! - **Base URL**: `https://api.twilio.com/2010-04-01/Accounts/{AccountSid}`
//! - **State & Secrets**: Credentials are stored via `Secrets<TwilioConfig>`.
//! - **Transport**: Uses `weil_rs::http::HttpClient` for HTTP requests.
//! - **MCP Surface**: The `tools()` method exposes JSON tool specs for agentic use;
//!   `prompts()` is a placeholder for future prompt templates.
//!
//! ## Supported Operations
//! - `send_sms`: Create an outbound SMS via `POST /Messages.json`.
//! - `send_voice_note`: Initiate a voice call with a TwiML URL via `POST /Calls.json`.
//! - `get_messages`: List recent messages via `GET /Messages.json` (optional `PageSize`).
//! - `get_account_info`: Fetch account metadata via `GET /Accounts/{sid}.json`.
//!
//! ## Notes
//! - This file adds **documentation only**; there are **no functional code changes**.
//! - Ensure your Twilio credentials are configured in `Secrets<TwilioConfig>` at deploy time.

use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use weil_macros::{WeilType, constructor, mutate, query, smart_contract};
use weil_rs::config::Secrets;
use weil_rs::http::{HttpClient, HttpMethod};

/// Twilio credentials and default sender configuration.
///
/// These values are stored securely via `Secrets<TwilioConfig>` and are
/// required to construct the HTTP Basic authorization header and account
/// base URL.
/// - `twilio_account_sid`: Your Twilio Account SID (e.g., `ACxxxxxxxx...`)
/// - `twilio_auth_token`: Your Twilio Auth Token
/// - `twilio_phone_number`: Default Twilio phone number (E.164), used as a sender in many flows
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct TwilioConfig {
    twilio_account_sid: String,
    twilio_auth_token: String,
    twilio_phone_number: String,
}

/// Shape of a Twilio Message resource as returned by `/Messages.json`.
///
/// Not all possible Twilio fields are represented—only common ones used by the contract.
#[derive(Debug, Serialize, Deserialize)]
pub struct TwilioMessage {
    /// Unique SID of the message (e.g., `SMxxxxxxxx`)
    sid: String,
    /// Body text of the message
    body: String,
    /// Sender (E.164)
    from: String,
    /// Recipient (E.164)
    to: String,
    /// Message status (e.g., `queued`, `sent`, `delivered`, `failed`)
    status: String,
    /// Creation timestamp (Twilio-formatted string)
    date_created: String,
}

/// Shape of a Twilio Call resource as returned by `/Calls.json`.
///
/// Only a subset of fields are modeled here.
#[derive(Debug, Serialize, Deserialize)]
pub struct TwilioCall {
    /// Unique SID of the call (e.g., `CAxxxxxxxx`)
    sid: String,
    /// Caller (E.164)
    from: String,
    /// Callee (E.164)
    to: String,
    /// Call status (e.g., `queued`, `ringing`, `in-progress`, `completed`)
    status: String,
    /// Call duration in seconds (string per Twilio API)
    duration: Option<String>,
    /// Creation timestamp (Twilio-formatted string)
    date_created: String,
}

/// Subset of Twilio Account metadata returned by `/{AccountSid}.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Friendly (human-readable) account name
    friendly_name: String,
    /// Account status (e.g., `active`, `suspended`, `closed`)
    status: String,
    /// Account SID (e.g., `ACxxxxxxxx`)
    sid: String,
}

/// Public MCP trait surface for Twilio operations.
///
/// All methods either mutate state on Twilio (SMS / Calls) or query resources
/// (messages / account info). `tools()` and `prompts()` expose machine-readable
/// metadata for agentic orchestration.
trait Twilio {
    /// Construct a new contract state with empty `Secrets<TwilioConfig>`.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Send an outbound SMS using Twilio's `POST /Messages.json`.
    ///
    /// - `from`: E.164 sender (often your Twilio number).
    /// - `to`: E.164 recipient.
    /// - `body`: Text content.
    ///
    /// Returns a success string mentioning the created Message SID if parsing succeeds,
    /// otherwise returns the raw Twilio response body for diagnostics.
    async fn send_sms(&mut self, from: String, to: String, body: String) -> Result<String, String>;

    /// Initiate a voice call using Twilio's `POST /Calls.json` with a TwiML URL.
    ///
    /// - `from`: E.164 caller (Twilio number).
    /// - `to`: E.164 callee.
    /// - `twiml_url`: Publicly reachable URL that returns TwiML `<Response>`.
    ///
    /// Returns a success string with Call SID if the response parses as `TwilioCall`,
    /// otherwise returns the raw response for diagnostics.
    async fn send_voice_note(
        &mut self,
        from: String,
        to: String,
        twiml_url: String,
    ) -> Result<String, String>;

    /// Retrieve recent messages via `GET /Messages.json`.
    ///
    /// - `limit`: Optional `PageSize` (Twilio will cap/interpret as supported).
    ///
    /// Returns a vector of `TwilioMessage`. Errors include HTTP or JSON parse issues.
    async fn get_messages(&self, limit: Option<u32>) -> Result<Vec<TwilioMessage>, String>;

    /// Fetch account metadata via `GET /Accounts/{AccountSid}.json`.
    async fn get_account_info(&self) -> Result<AccountInfo, String>;

    /// JSON schema describing callable tools for MCP/agent integrations.
    fn tools(&self) -> String;

    /// Placeholder for prompt templates used by agentic flows.
    fn prompts(&self) -> String;
}

/// Contract state holding Twilio credentials in secret storage.
#[derive(Serialize, Deserialize, WeilType)]
pub struct TwilioContractState {
    /// Secrets wrapper containing `TwilioConfig`
    secrets: Secrets<TwilioConfig>,
}

impl TwilioContractState {
    /// Build an HTTP `Authorization: Basic ...` header from `account_sid:auth_token`.
    ///
    /// Twilio uses Basic auth; the payload is Base64-encoded.
    fn create_auth_header(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let credentials = format!("{}:{}", config.twilio_account_sid, config.twilio_auth_token);
        let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
        Ok(format!("Basic {}", encoded))
    }

    /// Compute the base URL for this account's v2010 API.
    ///
    /// Example:
    /// `https://api.twilio.com/2010-04-01/Accounts/{AccountSid}`
    fn get_base_url(&self) -> Result<String, String> {
        let config = self.secrets.config();
        Ok(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}",
            config.twilio_account_sid
        ))
    }
}

#[smart_contract]
impl Twilio for TwilioContractState {
    /// Initialize an empty contract state with a new `Secrets<TwilioConfig>` container.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            secrets: Secrets::<TwilioConfig>::new(),
        })
    }

    /// Send an SMS via Twilio `POST /Messages.json`.
    ///
    /// **Form fields**: `From`, `To`, `Body`  
    /// **Auth**: `Authorization: Basic <base64(account_sid:auth_token)>`
    ///
    /// Returns `"SMS sent successfully. SID: <sid>"` if the JSON decodes into `TwilioMessage`,
    /// otherwise returns the raw response string for easier debugging of Twilio-side errors.
    #[mutate]
    async fn send_sms(&mut self, from: String, to: String, body: String) -> Result<String, String> {
        let url = format!("{}/Messages.json", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        // Create form data for Twilio API
        let mut form_data = HashMap::new();
        form_data.insert("From".to_string(), from);
        form_data.insert("To".to_string(), to);
        form_data.insert("Body".to_string(), body);

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .form(form_data) // This will properly encode form data in the body
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        // Try to parse the response to check for success
        match serde_json::from_str::<TwilioMessage>(&response_text) {
            Ok(message) => Ok(format!("SMS sent successfully. SID: {}", message.sid)),
            Err(_) => {
                // Return the raw response for debugging
                Ok(format!("SMS response: {}", response_text))
            }
        }
    }

    /// Initiate a voice call via Twilio `POST /Calls.json` using a TwiML URL.
    ///
    /// **Form fields**: `From`, `To`, `Url` (TwiML endpoint)  
    /// **Headers**: `Content-Type: application/x-www-form-urlencoded`  
    /// **Auth**: `Authorization: Basic <base64(account_sid:auth_token)>`
    ///
    /// Returns `"Call initiated successfully. SID: <sid>"` on parsed success, otherwise
    /// returns the raw Twilio response string.
    #[mutate]
    async fn send_voice_note(
        &mut self,
        from: String,
        to: String,
        twiml_url: String,
    ) -> Result<String, String> {
        let url = format!("{}/Calls.json", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        // Twilio expects form data, not JSON for calls
        let mut form_data = HashMap::new();
        form_data.insert("From".to_string(), from);
        form_data.insert("To".to_string(), to);
        form_data.insert("Url".to_string(), twiml_url);

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .form(form_data) // Use query for form data
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        // Try to parse the response to check for success
        match serde_json::from_str::<TwilioCall>(&response_text) {
            Ok(call) => Ok(format!("Call initiated successfully. SID: {}", call.sid)),
            Err(_) => {
                // If parsing fails, it might be an error response
                Ok(format!("Call request sent. Response: {}", response_text))
            }
        }
    }

    /// Fetch recent messages via `GET /Messages.json`.
    ///
    /// - When `limit` is `Some(n)`, it is passed as `PageSize=n`. Twilio may enforce its own caps.
    /// - The response is expected to have the shape `{ "messages": [ ... ] }`.
    ///
    /// Returns parsed `Vec<TwilioMessage>` or a descriptive parse error with the raw payload.
    #[query]
    async fn get_messages(&self, limit: Option<u32>) -> Result<Vec<TwilioMessage>, String> {
        let url = format!("{}/Messages.json", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut request_builder = HttpClient::request(&url, HttpMethod::Get).headers(headers);

        // Add query parameters if limit is specified
        if let Some(limit) = limit {
            let mut query_params = Vec::new();
            query_params.push(("PageSize".to_string(), limit.to_string()));
            request_builder = request_builder.query(query_params);
        }

        let response = request_builder.send().map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct MessagesResponse {
            messages: Vec<TwilioMessage>,
        }

        let messages_response: MessagesResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(messages_response.messages)
    }

    /// Retrieve account information via `GET /Accounts/{AccountSid}.json`.
    ///
    /// Returns `AccountInfo` or a parse error that includes the raw response string.
    #[query]
    async fn get_account_info(&self) -> Result<AccountInfo, String> {
        let url = format!("{}.json", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();
        serde_json::from_str::<AccountInfo>(&response_text).map_err(|err| {
            format!(
                "Failed to parse response: {}. Response was: {}",
                err, response_text
            )
        })
    }

    /// Machine-readable tool specifications for agentic orchestration (MCP).
    ///
    /// Exposes four functions: `send_sms`, `send_voice_note`, `get_messages`, `get_account_info`.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "send_sms",
      "description": "Send SMS message\n",
      "parameters": {
        "type": "object",
        "properties": {
          "from": {
            "type": "string",
            "description": "sender's number\n"
          },
          "to": {
            "type": "string",
            "description": "receiver's number\n"
          },
          "body": {
            "type": "string",
            "description": "content of the message\n"
          }
        },
        "required": [
          "from",
          "to",
          "body"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "send_voice_note",
      "description": "Make a phone call\n",
      "parameters": {
        "type": "object",
        "properties": {
          "from": {
            "type": "string",
            "description": "sender's number\n"
          },
          "to": {
            "type": "string",
            "description": "receiver's number\n"
          },
          "twiml_url": {
            "type": "string",
            "description": "twim url\n"
          }
        },
        "required": [
          "from",
          "to",
          "twiml_url"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_messages",
      "description": "Get message history with optional limit\n",
      "parameters": {
        "type": "object",
        "properties": {
          "limit": {
            "type": "number",
            "description": "max number of entries\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_account_info",
      "description": "Get account information\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  }
]"#
        .to_string()
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
