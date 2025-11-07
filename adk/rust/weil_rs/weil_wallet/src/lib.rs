//! # WeilClient — SDK client for WeilChain Applet execution
//!
//! This module exposes two primary client types:
//!
//! - [`WeilClient`]: a reusable HTTP client bound to a wallet, with concurrency
//!   control. It can execute smart-contract (applet) methods on arbitrary contracts,
//!   or create a per-contract handle via [`WeilClient::to_contract_client`].
//! - [`WeilContractClient`]: a thin wrapper around [`WeilClient`] that pins a
//!   specific [`ContractId`], offering `execute` and `execute_with_streaming`.
//!
//! ## How it works
//! 1. Build an [`ExecuteArgs`] payload for the target applet method.
//! 2. Construct a [`TransactionHeader`] with a nonce and addressing metadata.
//! 3. Canonicalize the payload (sorted `BTreeMap`), JSON-encode, and **sign** with
//!    the wallet’s secp256k1 key.
//! 4. Wrap into a [`BaseTransaction`] with a default TTL.
//! 5. Submit via the platform API, optionally obtaining a streaming response
//!    ([`streaming::ByteStream`]).
//!
//! ## Concurrency
//! Outbound HTTP submissions are gated by a `Semaphore` (default concurrency is
//! [`DEFAULT_CONCURRENCY`]). Clone the client freely; it is internally `Arc`
//! managed and safe to use across async tasks.

use api::{
    request::{SubmitTxnRequest, Transaction, UserTransaction, Verifier},
    PlatformApi,
};
use constants::{DEFAULT_CONCURRENCY, DEFAULT_TRANSACTION_TTL};
use contract::ContractId;
use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use std::{future::Future, sync::Arc};
use streaming::ByteStream;
use tokio::sync::Semaphore;
use transaction::{value_to_btreemap, BaseTransaction, TransactionHeader, TransactionResult};
use utils::{current_time_millis, get_address_from_public_key};
use wallet::Wallet;

pub mod api;
pub mod constants;
pub mod contract;
pub mod errors;
pub mod streaming;
pub mod transaction;
pub mod utils;
pub mod wallet;

/// High-level client for interacting with **WeilChain** applet methods.
///
/// Internally wraps a `reqwest::Client`, a signer [`Wallet`], and a concurrency
/// limiter. The whole struct is `Clone` (via `Arc`) and intended to be reused
/// across tasks.
#[derive(Clone)]
pub struct WeilClient {
    http_client: Client,
    wallet: Arc<Wallet>,
    semaphore: Arc<Semaphore>,
}

impl WeilClient {
    /// Construct a new [`WeilClient`].
    ///
    /// # Arguments
    /// - `wallet`: caller’s signing identity.
    /// - `concurrency`: optional maximum number of concurrent HTTP submissions.
    ///   Defaults to [`DEFAULT_CONCURRENCY`].
    ///
    /// # Notes
    /// - TLS certificate verification is disabled (`danger_accept_invalid_certs(true)`),
    ///   which is convenient for dev/test environments with self-signed certs.
    pub fn new(wallet: Wallet, concurrency: Option<usize>) -> Result<Self, anyhow::Error> {
        Ok(Self {
            http_client: Client::builder()
                .danger_accept_invalid_certs(true)
                .build()?,
            wallet: Arc::new(wallet),
            semaphore: Arc::new(Semaphore::new(match concurrency {
                Some(concurrency) => concurrency,
                None => DEFAULT_CONCURRENCY, // default value
            })),
        })
    }

    /// Create a [`WeilContractClient`] bound to a specific [`ContractId`].
    pub fn to_contract_client(&self, contract_id: ContractId) -> WeilContractClient {
        WeilContractClient {
            contract_id,
            client: self.clone(),
        }
    }

    /// Execute a contract method and wait for the normal (non-streaming) result.
    ///
    /// Convenience wrapper around [`WeilContractClient::execute`].
    pub async fn execute(
        &self,
        contract_id: ContractId,
        method_name: String,
        method_args: String,
    ) -> anyhow::Result<TransactionResult> {
        let resp = self
            .to_contract_client(contract_id)
            .execute(method_name, method_args)
            .await?;

        Ok(resp)
    }

    /// Execute a contract method with **streaming** response semantics.
    ///
    /// Convenience wrapper around [`WeilContractClient::execute_with_streaming`].
    pub async fn execute_with_streaming(
        &self,
        contract_id: ContractId,
        method_name: String,
        method_args: String,
    ) -> anyhow::Result<ByteStream> {
        let resp = self
            .to_contract_client(contract_id)
            .execute_with_streaming(method_name, method_args)
            .await?;

        Ok(resp)
    }
}

/// Canonicalized execution arguments for smart-contract invocation.
///
/// These fields are submitted to the platform API inside a signed transaction.
#[derive(Clone, Serialize)]
struct ExecuteArgs {
    /// Target contract/applet identifier.
    contract_address: ContractId,
    /// Exported method to call on the contract.
    contract_method: Arc<String>,
    /// JSON-encoded input payload (if any), wrapped for cheap cloning.
    contract_input_bytes: Option<Arc<String>>,
}

/// Per-contract view over a [`WeilClient`], used to call methods on a single applet.
#[derive(Clone)]
pub struct WeilContractClient {
    contract_id: ContractId,
    client: WeilClient,
}

impl WeilContractClient {
    /// Execute an exported method of the bound applet (non-streaming).
    ///
    /// Builds, signs, and submits a transaction; resolves to a [`TransactionResult`].
    pub async fn execute(
        &self,
        method_name: String,
        method_args: String,
    ) -> anyhow::Result<TransactionResult> {
        let (base_txn, signature, args) = self.sign_and_construct_txn(method_name, method_args)?;

        let resp = self
            .hit_api(
                signature,
                &base_txn,
                args,
                |payload: SubmitTxnRequest, http_client: Client| {
                    PlatformApi::submit_transaction(payload, http_client)
                },
            )
            .await;

        resp
    }

    /// Execute an exported method of the bound applet and return a **streaming** response.
    ///
    /// Suitable for methods that produce incremental output (`ByteStream`).
    pub async fn execute_with_streaming(
        &self,
        method_name: String,
        method_args: String,
    ) -> anyhow::Result<ByteStream> {
        let (base_txn, signature, args) = self.sign_and_construct_txn(method_name, method_args)?;

        let resp = self
            .hit_api(
                signature,
                &base_txn,
                args,
                |payload: SubmitTxnRequest, http_client: Client| {
                    PlatformApi::submit_transaction_with_streaming(payload, http_client)
                },
            )
            .await;

        resp
    }

    /// Construct and sign the base transaction for a method call.
    ///
    /// - Derives `from_addr` from the wallet's secp256k1 public key (`SHA-256` hex).
    /// - Uses current time in ms as the `nonce`.
    /// - Resolves the target `weilpod_counter` from the [`ContractId`].
    /// - Canonicalizes and signs the payload via [`Self::sign_execute_args`].
    fn sign_and_construct_txn(
        &self,
        method_name: String,
        method_args: String,
    ) -> Result<(BaseTransaction, String, ExecuteArgs), anyhow::Error> {
        let public_key = self.client.wallet.get_public_key();
        let from_addr = get_address_from_public_key(&public_key);
        let to_addr = from_addr.clone();
        let contract_id = self.contract_id.clone();
        let weilpod_counter = contract_id.pod_counter()?;
        let public_key = hex::encode(&public_key.serialize());

        let args = ExecuteArgs {
            contract_address: self.contract_id.clone(),
            contract_method: Arc::new(method_name),
            contract_input_bytes: Some(Arc::new(method_args)),
        };

        let nonce = current_time_millis() as usize;
        let mut txn_header =
            TransactionHeader::new(nonce, public_key, from_addr, to_addr, weilpod_counter);

        let signature = self.sign_execute_args(&txn_header, &args)?;
        txn_header.set_signature(signature.as_str());

        let base_txn = BaseTransaction::new(txn_header);

        Ok((base_txn, signature, args))
    }

    /// Canonicalize and **sign** the execute payload using the client wallet.
    ///
    /// - Builds a stable, sorted representation by converting the JSON payload
    ///   to a `BTreeMap` (`value_to_btreemap`) before serialization.
    /// - Signs the resulting bytes with `secp256k1` ECDSA via [`wallet::Wallet::sign`].
    fn sign_execute_args(
        &self,
        txn_header: &TransactionHeader,
        args: &ExecuteArgs,
    ) -> anyhow::Result<String> {
        let json_payload = json!({
            "nonce": txn_header.nonce,
            "from_addr": txn_header.from_addr,
            "to_addr": txn_header.to_addr,
            "user_txn": {
                "type": "SmartContractExecutor",
                "contract_address": args.contract_address,
                "contract_method": args.contract_method,
                "contract_input_bytes": args.contract_input_bytes,
            }
        });

        let json_payload_btreemap = value_to_btreemap(json_payload);
        let json_payload = serde_json::to_string(&json_payload_btreemap)?;
        let signature = self.client.wallet.sign(json_payload.as_bytes())?;

        Ok(signature)
    }

    /// Common submission path for both normal and streaming executions.
    ///
    /// Accepts an API function (from [`PlatformApi`]) to perform the actual HTTP
    /// request. Concurrency is limited by the client's internal semaphore.
    ///
    /// # Type parameters
    /// - `R`: API result type (e.g., [`TransactionResult`] or [`ByteStream`]).
    /// - `T`: future returned by the API function.
    /// - `F`: callable that takes a [`SubmitTxnRequest`] and a `reqwest::Client`,
    ///   returning `T`.
    async fn hit_api<R, T, F>(
        &self,
        signature: String,
        txn: &BaseTransaction,
        args: ExecuteArgs,
        api: F,
    ) -> Result<R, anyhow::Error>
    where
        T: Future<Output = Result<R, anyhow::Error>>,
        F: Fn(SubmitTxnRequest, Client) -> T,
    {
        // Re-encode the public key to ensure canonical on-wire format.
        let public_key = txn.header.parsed_public_key()?;
        let public_key = hex::encode(&public_key.serialize());

        let payload = SubmitTxnRequest {
            transaction: Transaction {
                is_xpod: false,
                txn_header: TransactionHeader {
                    nonce: txn.header.nonce,
                    public_key,
                    from_addr: txn.header.from_addr.clone(),
                    to_addr: txn.header.to_addr.clone(),
                    signature: Some(signature),
                    weilpod_counter: txn.header.weilpod_counter,
                    creation_time: current_time_millis() as u64,
                },
                verifier: Verifier {
                    ty: "DefaultVerifier".to_string(),
                },
                user_txn: UserTransaction {
                    ty: "SmartContractExecutor".to_string(),
                    contract_address: args.contract_address,
                    contract_method: args.contract_method,
                    contract_input_bytes: args.contract_input_bytes,
                },
            },
        };

        let _permit = self.client.semaphore.acquire().await.unwrap();

        let result = api(payload, self.client.http_client.clone()).await;

        drop(_permit);

        result
    }
}
