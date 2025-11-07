use crate::{contract::ContractId, transaction::TransactionHeader};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Verifier {
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UserTransaction {
    #[serde(rename = "type")]
    pub ty: String,
    pub contract_address: ContractId,
    pub contract_method: Arc<String>,
    pub contract_input_bytes: Option<Arc<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Transaction {
    pub is_xpod: bool,
    pub txn_header: TransactionHeader,
    pub verifier: Verifier,
    pub user_txn: UserTransaction,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SubmitTxnRequest {
    pub transaction: Transaction,
}
