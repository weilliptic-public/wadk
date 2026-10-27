use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};

#[derive(Debug, Serialize, Deserialize, WeilType, Clone)]
pub struct IncidentLog {
    pub incident_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub message: String,
    pub metadata: String,
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct IncidentLoggerContractState {
    logs: BTreeMap<String, Vec<IncidentLog>>,
}


trait IncidentLogger {
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    fn log_event(
        &mut self,
        incident_id: String,
        timestamp: String,
        event_type: String,
        message: String,
        metadata: String,
    ) -> Result<(), String>;

    fn get_incident_logs(
        &self,
        incident_id: String,
    ) -> Result<Vec<IncidentLog>, String>;

    fn list_incidents(&self) -> Result<Vec<String>, String>;
}


#[smart_contract]
impl IncidentLogger for IncidentLoggerContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            logs: BTreeMap::new(),
        })
    }

    #[mutate]
    fn log_event(
        &mut self,
        incident_id: String,
        timestamp: String,
        event_type: String,
        message: String,
        metadata: String,
    ) -> Result<(), String> {
        let entry = IncidentLog {
            incident_id: incident_id.clone(),
            timestamp,
            event_type,
            message,
            metadata,
        };

        self.logs
            .entry(incident_id)
            .or_insert_with(Vec::new)
            .push(entry);

        Ok(())
    }


    #[query]
    fn get_incident_logs(
        &self,
        incident_id: String,
    ) -> Result<Vec<IncidentLog>, String> {
        Ok(self.logs.get(&incident_id).cloned().unwrap_or_default())
    }

    #[query]
    fn list_incidents(&self) -> Result<Vec<String>, String> {
        Ok(self.logs.keys().cloned().collect())
    }
}
