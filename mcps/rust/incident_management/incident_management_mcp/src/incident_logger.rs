
use serde::{Deserialize, Serialize};
use anyhow::Result;
use weil_rs::runtime::Runtime;


#[derive(Debug, Serialize, Deserialize)]
pub struct IncidentLog {
    pub incident_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub message: String,
    pub metadata: String,
}


pub struct IncidentLoggerProxy {
    contract_id: String,
}

impl IncidentLoggerProxy {
    pub fn new(contract_id: String) -> Self {
        IncidentLoggerProxy {
            contract_id,
        }
    }
}

impl IncidentLoggerProxy {
    pub fn log_event(&self, incident_id: String, timestamp: String, event_type: String, message: String, metadata: String) -> Result<()> {

        #[derive(Debug, Serialize)]
        struct log_eventArgs {
            incident_id: String,
            timestamp: String,
            event_type: String,
            message: String,
            metadata: String,
        }

        let serialized_args = Some(serde_json::to_string(&log_eventArgs { incident_id, timestamp, event_type, message, metadata }).unwrap());

        let resp = Runtime::call_contract::<()>(
            self.contract_id.to_string(),
            "log_event".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }


    pub fn get_incident_logs(&self, indecident_id: String) -> Result<Vec<IncidentLog>> {

        #[derive(Debug, Serialize)]
        struct get_incident_logsArgs {
            incident_id: String,
        }

        let serialized_args = Some(serde_json::to_string(&get_incident_logsArgs { incident_id }).unwrap());

        let resp = Runtime::call_contract::<Vec<IncidentLog>>(
            self.contract_id.to_string(),
            "get_incident_logs".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

    pub fn list_incidents(&self) -> Result<Vec<String>> {
        let serialized_args = None;

        let resp = Runtime::call_contract::<Vec<String>>(
            self.contract_id.to_string(),
            "list_incidents".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

}
