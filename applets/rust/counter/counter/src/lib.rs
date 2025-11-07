use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};

trait Counter {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn get_count(&self) -> u32;
    async fn increment(&mut self);
    async fn set_value(&mut self, val: u32);
    async fn tools(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct CounterContractState {
    count: u32,
}

#[smart_contract]
impl Counter for CounterContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(CounterContractState { count: 0 })
    }

    #[query]
    async fn get_count(&self) -> u32 {
        self.count
    }

    #[mutate]
    async fn increment(&mut self) {
        self.count += 1
    }

    #[mutate]
    async fn set_value(&mut self, val: u32) {
        self.count = val
    }

    #[query]
    async fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "get_count",
      "description": "",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "increment",
      "description": "",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "set_value",
      "description": "",
      "parameters": {
        "type": "object",
        "properties": {
          "val": {
            "type": "number",
            "description": ""
          }
        },
        "required": [
          "val"
        ]
      }
    }
  }
]"#
        .to_string()
    }
}
