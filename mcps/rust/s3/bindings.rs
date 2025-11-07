
use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::collections::streaming::ByteStream;
use weil_rs::config::Secrets;
use weil_rs::webserver::WebServer;


#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct S3Config {
    access_key_id: String,
    secret_access_key: String,
    region: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct S3ObjectInfo {
    key: String,
    size: u64,
    last_modified: String,
}

trait S3 {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn upload(&self, bucket: String, key: String, data: Vec<u8>) -> Result<(), String>;
    async fn download(&self, bucket: String, key: String) -> Result<String, String>;
    async fn list_objects(&self, bucket: String, prefix: Option<String>) -> Result<Vec<S3ObjectInfo>, String>;
    async fn delete(&self, bucket: String, key: String) -> Result<(), String>;
    fn tools(&self) -> String;
    fn prompts(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct S3ContractState {
    // define your contract state here!
    secrets: Secrets<S3Config>,
}

#[smart_contract]
impl S3 for S3ContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        unimplemented!();
    }


    #[query]
    async fn upload(&self, bucket: String, key: String, data: Vec<u8>) -> Result<(), String> {
        unimplemented!();
    }

    #[query]
    async fn download(&self, bucket: String, key: String) -> Result<String, String> {
        unimplemented!();
    }

    #[query]
    async fn list_objects(&self, bucket: String, prefix: Option<String>) -> Result<Vec<S3ObjectInfo>, String> {
        unimplemented!();
    }

    #[query]
    async fn delete(&self, bucket: String, key: String) -> Result<(), String> {
        unimplemented!();
    }


    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "upload",
      "description": "Uploads a file to the specified S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          },
          "data": {
            "type": "array",
            "description": "The file contents as bytes\n"
          }
        },
        "required": [
          "bucket",
          "key",
          "data"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "download",
      "description": "Downloads a file from the specified S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          }
        },
        "required": [
          "bucket",
          "key"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_objects",
      "description": "Lists objects in the specified S3 bucket (optionally under a prefix)\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "prefix": {
            "type": "string",
            "description": "The prefix to filter objects (optional)\n"
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete",
      "description": "Deletes an object from the specified S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          }
        },
        "required": [
          "bucket",
          "key"
        ]
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

