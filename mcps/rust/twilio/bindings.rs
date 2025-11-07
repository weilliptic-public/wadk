
use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::collections::streaming::ByteStream;
use weil_rs::config::ConfigManager;


#[derive(Debug, Serialize, Deserialize)]
pub struct TwilioConfig {
    twilio_account_sid: String,
    twilio_auth_token: String,
    twilio_phone_number: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwilioMessage {
    sid: String,
    body: String,
    from: String,
    to: String,
    status: String,
    date_created: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwilioCall {
    sid: String,
    from: String,
    to: String,
    status: String,
    duration: Option<String>,
    date_created: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    friendly_name: String,
    status: String,
    sid: String,
}

trait Twilio {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn send_sms(&mut self, from: String, to: String, body: String) -> Result<(), String>;
    async fn send_voice_note(&mut self, from: String, to: String, twiml_url: String) -> Result<(), String>;
    async fn get_messages(&self, limit: Option<u32>) -> Result<Vec<TwilioMessage>, String>;
    async fn get_account_info(&self) -> Result<AccountInfo, String>;
    fn tools(&self) -> String;
    fn prompts(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct TwilioContractState {
    // define your contract state here!
}

#[smart_contract]
impl Twilio for TwilioContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        unimplemented!();
    }


    #[mutate]
    async fn send_sms(&mut self, from: String, to: String, body: String) -> Result<(), String> {
        unimplemented!();
    }

    #[mutate]
    async fn send_voice_note(&mut self, from: String, to: String, twiml_url: String) -> Result<(), String> {
        unimplemented!();
    }

    #[query]
    async fn get_messages(&self, limit: Option<u32>) -> Result<Vec<TwilioMessage>, String> {
        unimplemented!();
    }

    #[query]
    async fn get_account_info(&self) -> Result<AccountInfo, String> {
        unimplemented!();
    }


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
]"#.to_string()
    }


    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#.to_string()
    }
}

