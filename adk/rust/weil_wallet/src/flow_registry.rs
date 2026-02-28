use crate::{contract::ContractId, wallet::Wallet, WeilClient, WeilContractClient};
use serde::Serialize;
use weil_rs::errors::WeilError;

pub struct FlowRegistryClient {
    client: WeilContractClient,
}

impl FlowRegistryClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(FlowRegistryClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    /// Retrieves the execution context for a specific flow and namespace.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace to which the flow belongs.
    /// * `flow_id` - The identifier for the flow whose execution context is being requested.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(String))` - The execution context if found, serialized as a JSON string.
    /// * `Ok(None)` - If there is no execution context found for the given namespace and flow_id.
    /// * `Err(anyhow::Error)` - If an error occurs during the process, such as contract execution or deserialization failure.
    ///
    /// # Example
    ///
    /// ```
    /// let context = flow_registry_client.get_execution_context("my_namespace".to_string(), "my_flow".to_string()).await?;
    /// if let Some(ctx) = context {
    ///     println!("Execution context: {}", ctx);
    /// } else {
    ///     println!("No execution context found.");
    /// }
    /// ```
    pub async fn get_execution_context(
        &self,
        namespace: String,
        flow_id: String,
    ) -> Result<Option<String>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
        }

        let args = Args { namespace, flow_id };

        // Executes the "get_execution_context" contract method with the provided arguments.
        let resp = self
            .client
            .execute(
                "get_execution_context".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        // Deserialize the transaction result, which is an encoded Result<String, WeilError>
        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        // The `result` string is expected to encode an Option<String>; parse it accordingly.
        let result = serde_json::from_str::<Option<String>>(&result)?;

        Ok(result)
    }

    /// Persists the execution context for a given namespace and flow ID.
    ///
    /// This method serializes the provided `namespace`, `flow_id`, and execution context (`ctx`)
    /// as arguments and executes the "persist_execution_context" contract method.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace under which the flow is registered.
    /// * `flow_id` - The identifier of the flow.
    /// * `ctx` - The execution context to be persisted, serialized as a JSON string.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If persistence is successful.
    /// * `Err(anyhow::Error)` - If an error occurs during contract execution or (de)serialization.
    ///
    /// # Example
    ///
    /// ```
    /// flow_registry_client
    ///     .persist_execution_context(
    ///         "my_namespace".to_string(),
    ///         "my_flow".to_string(),
    ///         "{\"step\":1}".to_string(),
    ///     )
    ///     .await?;
    /// println!("Execution context persisted.");
    /// ```
    pub async fn persist_execution_context(
        &self,
        namespace: String,
        flow_id: String,
        ctx: String,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
            ctx: String,
        }

        let args = Args {
            namespace,
            flow_id,
            ctx,
        };

        // Execute the "persist_execution_context" contract method
        let resp = self
            .client
            .execute(
                "persist_execution_context".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        // Deserialize the transaction result
        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        // The result should be a unit `()`, serialized as a string
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

    /// Deletes a flow execution context for the specified namespace and flow ID.
    ///
    /// This method sends a request to the backend contract to remove the execution context associated with the provided
    /// `namespace` and `flow_id`. On success, the context is permanently deleted.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace under which the context resides.
    /// * `flow_id` - The identifier for the flow whose context you wish to delete.
    ///
    /// # Examples
    /// ```
    /// flow_registry_client
    ///     .delete_context(
    ///         "example_namespace".to_string(),
    ///         "example_flow".to_string(),
    ///     )
    ///     .await?;
    /// println!("Execution context deleted.");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`anyhow::Error`] if the request to delete the context fails,
    /// or if response deserialization does not complete successfully.
    pub async fn delete_context(
        &self,
        namespace: String,
        flow_id: String,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
        }

        let args = Args { namespace, flow_id };

        // Execute the "delete_context" contract method
        let resp = self
            .client
            .execute(
                "delete_context".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        // Deserialize transaction result as Result<String, WeilError>
        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        // The result should be a unit `()`, serialized as a string
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }
}
