use crate::{
    mcp::sqs::*,
    runtime::{get_length_prefixed_bytes_from_string, read_bytes_from_memory},
};
use serde::{Deserialize, Serialize};

#[link(wasm_import_module = "env")]
extern "C" {
    fn sqs_list_queues(params: i32) -> i32;
    fn sqs_create_queue(params: i32) -> i32;
    fn sqs_delete_queue(params: i32) -> i32;
    fn sqs_send_messages(params: i32) -> i32;
    fn sqs_receive_messages(params: i32) -> i32;
    fn sqs_delete_messages(params: i32) -> i32;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListQueuesResponse {
    pub queues: Vec<String>,
    pub next_token: Option<String>,
}
pub struct SQS;

impl SQS {
    /// Creates a queue. Returns a queue URL.
    pub fn create_queue(params: CreateQueueParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sqs_create_queue(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Deletes a queue. Returns a queue URL.
    pub fn delete_queue(params: CreateQueueParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sqs_delete_queue(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Lists queues. Returns a JSON array of Queue URLs and a continuation token if there are more queues.
    pub fn list_queues(params: ListQueuesParams) -> Result<ListQueuesResponse, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sqs_list_queues(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let response: ListQueuesResponse = serde_json::from_str(&result)?;
        Ok(response)
    }

    /// Sends multiple message to the specified queue. Returns a JSON object with lists of successful and failed message IDs.
    pub fn send_messages(
        params: SendMessagesParams,
    ) -> Result<SendMessagesResponse, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sqs_send_messages(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let response: SendMessagesResponse = serde_json::from_str(&result)?;
        Ok(response)
    }

    /// Receives messages from the specified queue. Returns a vector of tuples of messages and their receipt handles.
    pub fn receive_messages(
        params: ReceiveMessagesParams,
    ) -> Result<ReceiveMessagesResponse, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sqs_receive_messages(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let response: ReceiveMessagesResponse = serde_json::from_str(&result)?;
        Ok(response)
    }

    /// Deletes multiple message, with the specified ids, from the specified queue.
    pub fn delete_messages(
        params: DeleteMessagesParams,
    ) -> Result<DeleteMessagesResponse, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sqs_delete_messages(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let response: DeleteMessagesResponse = serde_json::from_str(&result)?;
        Ok(response)
    }
}
