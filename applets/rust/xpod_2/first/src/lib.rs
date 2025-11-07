use serde::{Deserialize, Serialize};
use weil_macros::{callback, constructor, query, smart_contract, xpod, WeilType};
use weil_rs::{
    collections::{map::WeilMap, WeilId},
    runtime::Runtime,
};

trait First {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn health_check(&self) -> String;
    async fn counter(&self, id: String) -> Result<u32, String>;
    fn set_list_in_second(
        &mut self,
        contract_id: String,
        id: String,
        val: u8,
    ) -> Result<(), String>;
    fn set_list_in_second_callback(&mut self, xpod_id: String, result: Result<Vec<u8>, String>);
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct FirstContractState {
    xpod_mapping: WeilMap<String, String>, // xpod_id -> id
    total_mapping: WeilMap<String, u32>,   // id -> counter
}

#[smart_contract]
impl First for FirstContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(FirstContractState {
            xpod_mapping: WeilMap::new(WeilId(0)),
            total_mapping: WeilMap::new(WeilId(1)),
        })
    }

    #[query]
    async fn health_check(&self) -> String {
        "Success!".to_string()
    }

    #[query]
    async fn counter(&self, id: String) -> Result<u32, String> {
        let Some(counter) = self.total_mapping.get(&id) else {
            return Err(format!("id `{}` is not present", id));
        };

        Ok(counter)
    }

    #[xpod]
    fn set_list_in_second(
        &mut self,
        contract_id: String,
        id: String,
        val: u8,
    ) -> Result<(), String> {
        #[derive(Serialize)]
        struct Args<'a> {
            id: &'a str,
            val: u8,
        }

        let xpod_id = Runtime::call_xpod_contract(
            contract_id,
            "set_val".to_string(),
            Some(serde_json::to_string(&Args { id: &id, val }).unwrap()),
        )
        .map_err(|err| err.to_string())?;

        Runtime::debug_log(&format!("xpod_id is {}", xpod_id));

        if self.total_mapping.get(&id).is_none() {
            self.total_mapping.insert(id.to_string(), 0);
        }

        self.xpod_mapping.insert(xpod_id, id);

        Ok(())
    }

    #[callback(set_list_in_second)]
    fn set_list_in_second_callback(&mut self, xpod_id: String, result: Result<Vec<u8>, String>) {
        Runtime::debug_log(&format!("The list is {:?} for xpod_id {}", result, xpod_id));

        if let Ok(_) = result {
            let Some(id) = self.xpod_mapping.get(&xpod_id) else {
                return;
            };

            let Some(counter) = self.total_mapping.get(&id) else {
                unreachable!()
            };

            self.total_mapping.insert(id, counter + 1);
        }
    }
}
