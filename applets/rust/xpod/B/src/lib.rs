use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};

pub trait B {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    fn generate_greetings_1(&self, name: String) -> String;
    fn generate_greetings_2(&self, name: String) -> String;
    fn generate_greetings_3(&mut self, name: String) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct BContractState {}

#[smart_contract]
impl B for BContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(BContractState {})
    }

    #[query]
    fn generate_greetings_1(&self, name: String) -> String {
        format!("From 1: Hello, {}", name)
    }

    #[query]
    fn generate_greetings_2(&self, name: String) -> String {
        format!("From 2: Hello, {}", name)
    }

    #[mutate]
    fn generate_greetings_3(&mut self, name: String) -> String {
        format!("From 3: Hello, {}", name)
    }
}
