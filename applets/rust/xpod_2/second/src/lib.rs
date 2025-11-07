use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::collections::{map::WeilMap, WeilId};

trait Second {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn get_list(&self, id: String) -> Result<Vec<u8>, String>;
    async fn set_val(&mut self, id: String, val: u8) -> Vec<u8>;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct SecondContractState {
    map: WeilMap<String, Vec<u8>>,
}

#[smart_contract]
impl Second for SecondContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(SecondContractState {
            map: WeilMap::new(WeilId(0)),
        })
    }

    #[query]
    async fn get_list(&self, id: String) -> Result<Vec<u8>, String> {
        let Some(val) = self.map.get(&id) else {
            return Err(format!("id `{}` not present", id));
        };

        Ok(val)
    }

    #[mutate]
    async fn set_val(&mut self, id: String, val: u8) -> Vec<u8> {
        let Some(mut list) = self.map.get(&id) else {
            self.map.insert(id, vec![val]);

            return vec![val];
        };

        list.push(val);

        self.map.insert(id, list.clone());

        list
    }
}
