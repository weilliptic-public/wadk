use serde::{Deserialize, Serialize};
use weil_macros::{callback, constructor, query, smart_contract, xpod, WeilType};
use weil_rs::runtime::Runtime;

pub trait A {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    fn greetings(&self, name: String, contract_addr: String) -> Result<String, String>;
    fn x_greetings(&mut self, name: String, contract_addr: String) -> Result<(), String>;
    fn x_greetings_callback(&mut self, resp: Result<String, String>);
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct AContractState {
    // define your contract state here!
    prefix: String,
}

#[smart_contract]
impl A for AContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(AContractState {
            prefix: String::from("A"),
        })
    }

    #[query]
    fn greetings(&self, name: String, contract_addr: String) -> Result<String, String> {
        #[derive(Serialize)]
        struct Args {
            name: String,
        }

        let args = Args { name };

        Runtime::call_contract::<String>(
            contract_addr,
            "generate_greetings_1".to_string(),
            Some(serde_json::to_string(&args).unwrap()),
        )
        .map_err(|err| err.to_string())
    }

    #[xpod]
    fn x_greetings(&mut self, name: String, contract_addr: String) -> Result<(), String> {
        #[derive(Serialize)]
        struct Args {
            name: String,
        }

        let args = Args { name };

        let _ = Runtime::call_xpod_contract(
            contract_addr,
            "generate_greetings_1".to_string(),
            Some(serde_json::to_string(&args).unwrap()),
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    #[callback(x_greetings)]
    fn x_greetings_callback(&mut self, result: Result<String, String>) {
        Runtime::debug_log(&format!("xpod greetings result is {:?}", result));
    }
}
