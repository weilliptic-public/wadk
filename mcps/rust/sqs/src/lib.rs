//! # AWS SQS Smart Contract
//!
//! This module defines a `smart_contract` implementation that integrates with
//! AWS Simple Queue Service (SQS) using the `weil_rs` framework.
//!
//! It provides a complete interface to perform queue management operations like
//! creating, deleting, listing queues, sending, receiving, and deleting messages.
//!
//! The contract encapsulates credentials management using `Secrets<Config>` and
//! defines callable functions (tools) for each SQS operation.
//!
//! ## Key Features
//! - Secure AWS credential handling via `Secrets`.
//! - Fully asynchronous query methods for SQS operations.
//! - Smart contract trait `AwsSQS` implementing standard queue management methods.
//! - Integration with the Weil MCP framework for on-chain/off-chain interoperability.

use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::config::Secrets;
use weil_rs::sqs::SQS;
use weil_rs::mcp::sqs::{
    CreateQueueParams, Credentials, DeleteMessagesParams, ListQueuesParams, ReceiveMessagesParams,
    SendMessagesParams,
};

/// AWS SQS configuration data.
///
/// This struct holds the credentials and region configuration
/// used to authenticate SQS operations.
///
/// # Fields
/// - `access_key_id`: The AWS Access Key ID.
/// - `secret_access_key`: The AWS Secret Access Key.
/// - `region`: The AWS region where the queues reside.
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct Config {
    access_key_id: String,
    secret_access_key: String,
    region: String,
}

/// Trait defining all AWS SQS operations exposed by the contract.
///
/// Each method corresponds to an SQS API call or a logical wrapper around multiple calls.
trait AwsSQS {
    /// Creates a new instance of the smart contract.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Lists all queues available in the AWS account.
    ///
    /// # Arguments
    /// * `max_results` - Optional. Limits the number of queues returned.
    /// * `next_token` - Optional. Used for pagination.
    ///
    /// # Returns
    /// A tuple containing:
    /// - A list of queue URLs.
    /// - An optional continuation token for pagination.
    async fn list_queues(
        &self,
        max_results: Option<i32>,
        next_token: Option<String>,
    ) -> Result<(Vec<String>, Option<String>), String>;

    /// Creates a new SQS queue.
    ///
    /// If the queue name ends with `.fifo`, a FIFO queue is created.
    ///
    /// # Arguments
    /// * `name` - The name of the queue.
    ///
    /// # Returns
    /// The URL of the created queue.
    async fn create_queue(&self, name: String) -> Result<String, String>;

    /// Deletes an existing SQS queue.
    ///
    /// # Arguments
    /// * `name` - The name of the queue to delete.
    ///
    /// # Returns
    /// An optional string indicating success or additional information.
    async fn delete_queue(&self, name: String) -> Result<Option<String>, String>;

    /// Sends one or more messages to the specified queue.
    ///
    /// # Arguments
    /// * `queue` - The name of the target queue.
    /// * `messages` - A list of messages to send.
    ///
    /// # Returns
    /// A list of messages that failed to send.
    async fn send_messages(&self, queue: String, messages: Vec<String>)
        -> Result<Vec<String>, String>;

    /// Receives messages from the specified queue.
    ///
    /// # Arguments
    /// * `queue` - The queue name.
    /// * `max_results` - Optional maximum number of messages to receive (1–10).
    ///
    /// # Returns
    /// A list of `(message_body, receipt_handle)` pairs.
    async fn receive_messages(
        &self,
        queue: String,
        max_results: Option<i32>,
    ) -> Result<Vec<(String, String)>, String>;

    /// Receives and immediately deletes messages from a queue.
    ///
    /// # Arguments
    /// * `queue` - The name of the queue.
    /// * `max_results` - Optional maximum number of messages to process.
    ///
    /// # Returns
    /// A tuple containing:
    /// - A list of received message bodies.
    /// - A list of failed deletion handles.
    async fn receive_and_delete_messages(
        &self,
        queue: String,
        max_results: Option<i32>,
    ) -> Result<(Vec<String>, Vec<String>), String>;

    /// Deletes specific messages from a queue using their receipt handles.
    ///
    /// # Arguments
    /// * `queue` - The name of the queue.
    /// * `handles` - The list of receipt handles to delete.
    ///
    /// # Returns
    /// A list of messages that failed to delete.
    async fn delete_messages(&self, queue: String, handles: Vec<String>)
        -> Result<Vec<String>, String>;

    /// Returns a JSON string describing the available tool functions.
    ///
    /// Useful for LLM tool integration or reflection-based invocation.
    fn tools(&self) -> String;

    /// Returns the JSON prompt configuration (currently empty).
    fn prompts(&self) -> String;
}

/// The contract state for AWS SQS interaction.
///
/// Encapsulates the secure AWS credentials and any persistent state
/// related to queue operations.
#[derive(Serialize, Deserialize, WeilType)]
pub struct AwsSQSContractState {
    /// AWS credentials stored securely using `Secrets`.
    secrets: Secrets<Config>,
}

#[smart_contract]
impl AwsSQS for AwsSQSContractState {
    /// Constructor: Initializes the smart contract with empty secrets.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(AwsSQSContractState {
            secrets: Secrets::new(),
        })
    }

    /// Lists all available queues in AWS SQS.
    #[query]
    async fn list_queues(
        &self,
        max_results: Option<i32>,
        next_token: Option<String>,
    ) -> Result<(Vec<String>, Option<String>), String> {
        let config = self.secrets.config();
        let credentials = Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = ListQueuesParams {
            credentials,
            prefix: None,
            next_token,
            max_results,
        };
        let response = SQS::list_queues(params).map_err(|e| e.to_string())?;
        Ok((response.queues, response.next_token))
    }

    /// Creates a new queue with the given name.
    #[query]
    async fn create_queue(&self, name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let credentials = Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = CreateQueueParams { credentials, name };
        let resp = SQS::create_queue(params).map_err(|e| e.to_string())?;
        Ok(resp)
    }

    /// Deletes a queue by name.
    #[query]
    async fn delete_queue(&self, name: String) -> Result<Option<String>, String> {
        let config = self.secrets.config();
        let credentials = Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = CreateQueueParams { credentials, name };
        let resp = SQS::delete_queue(params).map_err(|e| e.to_string())?;
        Ok(Some(resp))
    }

    /// Sends messages to the given queue.
    #[query]
    async fn send_messages(
        &self,
        queue: String,
        messages: Vec<String>,
    ) -> Result<Vec<String>, String> {
        let config = self.secrets.config();
        let credentials = Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = SendMessagesParams {
            credentials,
            queue,
            messages,
        };
        let response = SQS::send_messages(params).map_err(|e| e.to_string())?;
        Ok(response.failed)
    }

    /// Receives messages from a queue (without deleting).
    #[query]
    async fn receive_messages(
        &self,
        queue: String,
        max_results: Option<i32>,
    ) -> Result<Vec<(String, String)>, String> {
        let config = self.secrets.config();
        let credentials = Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = ReceiveMessagesParams {
            credentials,
            queue,
            max_results,
        };
        let response = SQS::receive_messages(params).map_err(|e| e.to_string())?;
        Ok(response.received)
    }

    /// Receives and deletes messages in one operation.
    #[query]
    async fn receive_and_delete_messages(
        &self,
        queue: String,
        max_results: Option<i32>,
    ) -> Result<(Vec<String>, Vec<String>), String> {
        let msg_and_handles = self.receive_messages(queue.clone(), max_results).await?;
        let messages: Vec<String> = msg_and_handles.iter().map(|(msg, _)| msg.clone()).collect();
        let handles: Vec<String> = msg_and_handles.iter().map(|(_, handle)| handle.clone()).collect();
        let deleted = self.delete_messages(queue, handles).await?;
        Ok((messages, deleted))
    }

    /// Deletes messages by their receipt handles.
    #[query]
    async fn delete_messages(
        &self,
        queue: String,
        handles: Vec<String>,
    ) -> Result<Vec<String>, String> {
        let config = self.secrets.config();
        let credentials = Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = DeleteMessagesParams {
            credentials,
            queue,
            handles,
        };
        let resp = SQS::delete_messages(params).map_err(|e| e.to_string())?;
        Ok(resp.failed)
    }


    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "create_queue",
      "description": "Creates a queue with the specified name. If the name ends with \".fifo\", a FIFO queue is created.\nReturns the URL of the created queue.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Name of the queue to create\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_queue",
      "description": "Deletes the queue with the specified name.\nReturns an optional string indicating success.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Name of the queue to create\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_queues",
      "description": "Lists the queues in the AWS account.\nSupports pagination via max_results and next_token parameters.\n\nReturns a tuple containing a list of queue names and an optional continuation token\nto be used in a subsequent call to retrieve the next set of results.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "max_results": {
            "type": "integer",
            "description": "Maximum number of queues to return (optional)\n"
          },
          "next_token": {
            "type": "string",
            "description": "Continuation token for pagination (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "send_messages",
      "description": "Sends a list of messages to the specified queue.\nThe list may contain a single element.\nReturns a vector with the indexes of messages whose sending failed.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "queue": {
            "type": "string",
            "description": "Name of the queue to which to send the messages.\n"
          },
          "messages": {
            "type": "array",
            "description": "The list of messages to send.\n"
          }
        },
        "required": [
          "queue",
          "messages"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "receive_messages",
      "description": "Receives messages from the specified queue.\nReturns a list of tuples of messages and their receipt handles (used for deletion).\n\nOptions include:\n- max_results: the maximum number of messages to return (default of 1 and up to 10).\n",
      "parameters": {
        "type": "object",
        "properties": {
          "queue": {
            "type": "string",
            "description": "Name of the queue from which to receive the messages.\n"
          },
          "max_results": {
            "type": "integer",
            "description": "Maximum number of messages to return (optional)\n"
          }
        },
        "required": [
          "queue"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "receive_and_delete_messages",
      "description": "Receives messages from the specified queue and immediately tries to delete them.\nReturns a tuple with\n- a list of all messages received and\n- a list of indexes of messages not deleted.\n\nOptions include:\n- max_results: the maximum number of messages to return (defaults of 1 and up to 10).\n",
      "parameters": {
        "type": "object",
        "properties": {
          "queue": {
            "type": "string",
            "description": "Name of the queue from which to receive the messages.\n"
          },
          "max_results": {
            "type": "integer",
            "description": "Maximum number of messages to return (optional)\n"
          }
        },
        "required": [
          "queue"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_messages",
      "description": "Deletes a batch of messages with the specified receipt handles from the specified queue.\nReturns a vector with the indexes of messages whose deletion failed.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "queue": {
            "type": "string",
            "description": "Name of the queue from which to delete the messages.\n"
          },
          "handles": {
            "type": "array",
            "description": "The receipt handles of messages to delete.\n"
          }
        },
        "required": [
          "queue",
          "handles"
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

