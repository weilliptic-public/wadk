
use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::collections::streaming::ByteStream;
use weil_rs::config::Secrets;
use weil_rs::webserver::WebServer;


#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct AuroraDBConfig {
    username: String,
    password: String,
    endpoint: String,
}

trait Aurora {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn run_query(&self, query_str: String, db_name: String) -> Result<Vec<String>, String>;
    async fn execute(&self, db_name: String, statement: String) -> Result<u64, String>;
    async fn run_query_and_save(&self, query_str: String, db_name: String, filename: String) -> Result<Vec<String>, String>;
    fn tools(&self) -> String;
    fn prompts(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct AuroraContractState {
    // define your contract state here!
    secrets: Secrets<AuroraDBConfig>,
}

#[smart_contract]
impl Aurora for AuroraContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        unimplemented!();
    }


    #[query]
    async fn run_query(&self, query_str: String, db_name: String) -> Result<Vec<String>, String> {
        unimplemented!();
    }

    #[query]
    async fn execute(&self, db_name: String, statement: String) -> Result<u64, String> {
        unimplemented!();
    }

    #[query]
    async fn run_query_and_save(&self, query_str: String, db_name: String, filename: String) -> Result<Vec<String>, String> {
        unimplemented!();
    }


    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "execute",
      "description": "This executes the statement in aurora provided in argument `statement` potentially mutating the rows of the database with name given by argument `db_name`.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "db_name": {
            "type": "string",
            "description": "the name of the database you want to run the query in\n"
          },
          "statement": {
            "type": "string",
            "description": "the statement you want to execute\n"
          }
        },
        "required": [
          "db_name",
          "statement"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_query",
      "description": "This runs a query provided in `query_str` in aurora on the database with name given by argument `db_name`.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "the query string that you want to run\n"
          },
          "db_name": {
            "type": "string",
            "description": "the name of the database you want to run the query in\n"
          }
        },
        "required": [
          "query_str",
          "db_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_query_and_save",
      "description": "saves the data returned by running a query provided in `query_str` in aurora on the database with name given by argument `db_name`.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "the query string that you want to run\n"
          },
          "db_name": {
            "type": "string",
            "description": "the name of the database you want to run the query in\n"
          },
          "filename": {
            "type": "string",
            "description": "the filename to save to\n"
          }
        },
        "required": [
          "query_str",
          "db_name",
          "filename"
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

