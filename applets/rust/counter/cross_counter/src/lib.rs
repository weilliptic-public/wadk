mod counter;

use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use crate::counter::Counter;


pub trait CrossCounter {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    fn fetch_counter_from(&self, contract_id: String) -> Result<usize,String>;
    fn increment_counter_of(&mut self, contract_id: String) -> Result<(),String>;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct CrossCounterContractState {
    // define your contract state here!
}

#[smart_contract]
impl CrossCounter for CrossCounterContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(CrossCounterContractState {})
    }

    #[query]
    fn fetch_counter_from(&self, contract_id: String) -> Result<usize,String> {
        let counter = Counter::new(contract_id);
        counter.get_count().map_err(|err| err.to_string())
    }

    #[mutate]
    fn increment_counter_of(&mut self, contract_id: String) -> Result<(),String> {
        let counter = Counter::new(contract_id);
        counter.increment().map_err(|err| err.to_string())
    }

}
