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
//!    the wallet's secp256k1 key.
//! 4. Wrap into a [`BaseTransaction`] with a default TTL.
//! 5. Submit via the platform API, optionally obtaining a streaming response
//!    ([`streaming::ByteStream`]).
//!
//! ## Concurrency
//! Outbound HTTP submissions are gated by a `Semaphore` (default concurrency is
//! [`DEFAULT_CONCURRENCY`]). Clone the client freely; it is internally `Arc`
//! managed and safe to use across async tasks.
//!
//! ## Multi-account
//! The wallet is held behind a `Mutex` so the active account can be switched
//! at runtime without recreating the client:
//! ```no_run
//! use weil_wallet::{WeilClient, wallet::SelectedAccount};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = WeilClient::from_wallet_file("wallet.wc", None)?;
//! client.set_account(&SelectedAccount::Derived(1)).await?; // switch to account 1
//! # Ok(())
//! # }
//! ```

use api::{
    request::{SubmitTxnRequest, Transaction, UserTransaction, Verifier},
    PlatformApi,
};
use constants::{DEFAULT_CONCURRENCY, SENTINEL_HOST};
use contract::ContractId;
use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use std::{future::Future, sync::Arc};
use streaming::ByteStream;
use tokio::sync::{Mutex, Semaphore};
use transaction::{value_to_btreemap, BaseTransaction, TransactionHeader, TransactionResult};
use utils::{current_time_millis, hash_sha256};
use wallet::{SelectedAccount, Wallet};

const AUDIT_APPLET_SVC_NAME: &str = "auditor::weil";

pub mod api;
pub mod constants;
pub mod contract;
pub mod errors;
pub mod flow_registry;
pub mod streaming;
pub mod transaction;
pub mod utils;
pub mod wallet;

/// High-level client for interacting with **WeilChain** applet methods.
///
/// Internally wraps a `reqwest::Client`, a signer [`Wallet`] (behind a `Mutex`
/// for multi-account support), and a concurrency limiter. The whole struct is
/// `Clone` (via `Arc`) and safe to share across tasks.
#[derive(Clone)]
pub struct WeilClient {
    http_client: Client,
    wallet: Arc<std::sync::Mutex<Wallet>>,
    semaphore: Arc<Semaphore>,
    audit_contract_id: Arc<Mutex<Option<ContractId>>>,
}

impl WeilClient {
    /// Construct a new [`WeilClient`].
    ///
    /// # Arguments
    /// - `wallet`: caller's signing identity.
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
            wallet: Arc::new(std::sync::Mutex::new(wallet)),
            semaphore: Arc::new(Semaphore::new(match concurrency {
                Some(c) => c,
                None => DEFAULT_CONCURRENCY,
            })),
            audit_contract_id: Arc::new(Mutex::new(None)),
        })
    }

    /// Construct a [`WeilClient`] from a `wallet.wc` file.
    /// No sentinel connection required for wallet construction.
    pub fn from_wallet_file<P: AsRef<std::path::Path>>(
        path: P,
        concurrency: Option<usize>,
    ) -> Result<Self, anyhow::Error> {
        let wallet = Wallet::from_wallet_file(path)?;
        Self::new(wallet, concurrency)
    }

    // ── Multi-account management ─────────────────────────────────────────────

    /// Switch the active account used for signing.
    ///
    /// Returns an error if the index is out of bounds.
    pub async fn set_account(&self, selected: &SelectedAccount) -> anyhow::Result<()> {
        let mut wallet = self.wallet.lock().unwrap();
        wallet.set_index(selected)
    }

    /// Return the number of derived (HD) accounts in the wallet.
    pub async fn derived_account_count(&self) -> usize {
        self.wallet.lock().unwrap().derived_account_count()
    }

    /// Return the number of externally added accounts in the wallet.
    pub async fn external_account_count(&self) -> usize {
        self.wallet.lock().unwrap().external_account_count()
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    /// Resolve and cache the audit applet contract address from the Sentinel API.
    async fn get_audit_contract_id(&self) -> anyhow::Result<ContractId> {
        let mut guard = self.audit_contract_id.lock().await;
        if let Some(ref id) = *guard {
            return Ok(id.clone());
        }
        let url = format!("{}/get_applet_address", SENTINEL_HOST);
        let body = json!({ "svc_name": AUDIT_APPLET_SVC_NAME });
        let resp = self.http_client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("get_applet_address failed: HTTP {} {}", status, text);
        }
        let data: serde_json::Value = resp.json().await?;
        let address = data
            .get("Ok")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "get_applet_address failed: {}",
                    data.get("Err")
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| data.to_string())
                )
            })?;
        let contract_id = ContractId::new(address.to_string())?;
        *guard = Some(contract_id.clone());
        Ok(contract_id)
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
        should_hide_args: Option<bool>,
        is_non_blocking: Option<bool>,
    ) -> anyhow::Result<TransactionResult> {
        let resp = self
            .to_contract_client(contract_id)
            .execute(method_name, method_args, should_hide_args, is_non_blocking)
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
        should_hide_args: Option<bool>,
    ) -> anyhow::Result<ByteStream> {
        let resp = self
            .to_contract_client(contract_id)
            .execute_with_streaming(method_name, method_args, should_hide_args)
            .await?;

        Ok(resp)
    }

    pub async fn audit(&self, log: String) -> anyhow::Result<String> {
        let contract_id = self.get_audit_contract_id().await?;
        let method_name = "audit";

        #[derive(Serialize)]
        struct AuditArgs {
            log: String,
            org: Option<String>
        }

        let org = self.wallet.lock().unwrap().org().map(|o| o.name.clone());

        let method_args = serde_json::to_string(&AuditArgs { log, org }).unwrap();

        let resp = self
            .execute(
                contract_id,
                method_name.to_string(),
                method_args,
                Some(false),
                Some(false),
            )
            .await?;

        let txn_result = resp.txn_result;

        Ok(txn_result)
    }

    pub async fn persist_receipt_for_commit(
        &self,
        commit_hash: String,
        receipt: String,
    )-> anyhow::Result<String>{
        let contract_id = self.get_audit_contract_id().await?;
        let method_name = "persist_receipt_for_commit";

        #[derive(Serialize)]
        struct PersistReceiptArgs {
            commit_hash: String,
            receipt: String,
        }

        let method_args = serde_json::to_string(&PersistReceiptArgs { commit_hash, receipt }).unwrap();

        let resp = self
            .execute(
                contract_id,
                method_name.to_string(),
                method_args,
                Some(false),
                Some(false),
            )
            .await?;

        let txn_result = resp.txn_result;

        Ok(txn_result)
    }

    /// Uploads a receipt (trace + transcript) to cloud storage via the `audit_me`
    /// contract's `validate_and_persist_receipt` method.
    ///
    /// The caller's org and subgroup are extracted from the wallet export so that
    /// the contract can verify identity using the caller's actual subgroup — not a
    /// hardcoded group name.
    pub async fn persist_receipt_in_s3(
        &self,
        content: String,
    ) -> anyhow::Result<String> {
        let contract_id = self.get_audit_contract_id().await?;
        let method_name = "validate_and_persist_receipt";

        #[derive(Serialize)]
        struct PersistReceiptInS3Args {
            org: Option<String>,
            subgroup: Option<String>,
            hash: String,
            content: String,
        }

        let hash = hex::encode(hash_sha256(content.as_bytes()));
        let wallet = self.wallet.lock().unwrap();
        let org = wallet.org().map(|o| o.name.clone());
        let subgroup = wallet.org().and_then(|o| o.subgroup.clone());
        drop(wallet);

        let method_args =
            serde_json::to_string(&PersistReceiptInS3Args { org, subgroup, hash, content })
                .unwrap();

        let resp = self
            .execute(
                contract_id,
                method_name.to_string(),
                method_args,
                Some(false),
                Some(false),
            )
            .await?;

        let txn_result = resp.txn_result;

        Ok(txn_result)
    }

    pub async fn index_prompt_in_bigquery(&self, prompt_text: String, receipts: Vec<String>) -> Result<(), anyhow::Error> {
        
        let contract_id = self.get_audit_contract_id().await?;
        let org = self.wallet.lock().unwrap().org().map(|o| o.name.clone());
        let prompt_id = hex::encode(hash_sha256(prompt_text.as_bytes()));
        let namespace = org.clone().unwrap_or_default();

        #[derive(Serialize)]
        struct Args {
            org: Option<String>,
            namespace: String,
            prompt_text: String,
            prompt_id: String,
            receipts: Vec<String>,
        }

        let args = Args { org, namespace, prompt_text, prompt_id, receipts };

        let _ = self.
            execute(
                contract_id,
                "index_prompt".to_string(),
                serde_json::to_string(&args).unwrap(),
                None, 
                None).await?;

        Ok(())
    }

    /// Indexes multiple prompts in a single on-chain transaction.
    ///
    /// Each `(prompt_text, receipts)` pair is converted into a `PromptEntry`
    /// with a SHA-256-derived `prompt_id` and the org name as `namespace`.
    /// The `audit_me` contract qualifies each `prompt_id` with the caller's
    /// wallet address before forwarding the batch to Sentinel.
    pub async fn batch_index_prompts(
        &self,
        prompts: Vec<(String, Vec<String>)>,
    ) -> Result<(), anyhow::Error> {
        let contract_id = self.get_audit_contract_id().await?;
        let org = self.wallet.lock().unwrap().org().map(|o| o.name.clone());
        let namespace = org.clone().unwrap_or_default();

        #[derive(Serialize)]
        struct PromptEntry {
            namespace: String,
            prompt_text: String,
            prompt_id: String,
            receipts: Vec<String>,
        }

        #[derive(Serialize)]
        struct Args {
            org: Option<String>,
            prompts: Vec<PromptEntry>,
        }

        let entries: Vec<PromptEntry> = prompts
            .into_iter()
            .map(|(prompt_text, receipts)| {
                let prompt_id = hex::encode(hash_sha256(prompt_text.as_bytes()));
                PromptEntry { namespace: namespace.clone(), prompt_text, prompt_id, receipts }
            })
            .collect();

        let args = Args { org, prompts: entries };

        let _ = self
            .execute(
                contract_id,
                "batch_index_prompts".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        Ok(())
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
    should_hide_args: bool,
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
        should_hide_args: Option<bool>,
        is_non_blocking: Option<bool>,
    ) -> anyhow::Result<TransactionResult> {
        let (base_txn, signature, args) = self.sign_and_construct_txn(
            method_name,
            method_args,
            should_hide_args.unwrap_or(true),
        )?;

        let resp = self
            .hit_api(
                signature,
                &base_txn,
                args,
                is_non_blocking.unwrap_or(false),
                |payload: SubmitTxnRequest, http_client: Client, is_non_blocking: bool| {
                    PlatformApi::submit_transaction(payload, http_client, is_non_blocking)
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
        should_hide_args: Option<bool>,
    ) -> anyhow::Result<ByteStream> {
        let (base_txn, signature, args) = self.sign_and_construct_txn(
            method_name,
            method_args,
            should_hide_args.unwrap_or(true),
        )?;

        let resp = self
            .hit_api(
                signature,
                &base_txn,
                args,
                false,
                |payload: SubmitTxnRequest, http_client: Client, _is_non_blocking: bool| {
                    PlatformApi::submit_transaction_with_streaming(payload, http_client)
                },
            )
            .await;

        resp
    }

    /// Construct and sign the base transaction for a method call.
    ///
    /// Locks the wallet mutex briefly to snapshot the current account's address,
    /// public key, and produce the signature, then releases the lock before any
    /// network I/O.
    fn sign_and_construct_txn(
        &self,
        method_name: String,
        method_args: String,
        should_hide_args: bool,
    ) -> Result<(BaseTransaction, String, ExecuteArgs), anyhow::Error> {
        let contract_id = self.contract_id.clone();
        let weilpod_counter = contract_id.pod_counter()?;

        let args = ExecuteArgs {
            contract_address: self.contract_id.clone(),
            contract_method: Arc::new(method_name),
            contract_input_bytes: Some(Arc::new(method_args)),
            should_hide_args,
        };

        // Lock the wallet only for the duration of address/key access and signing.
        let wallet = self.client.wallet.lock().unwrap();

        let from_addr = Arc::new(wallet.get_address().to_string());
        let to_addr = from_addr.clone();
        let public_key_hex = hex::encode(wallet.get_public_key().serialize());

        let nonce = current_time_millis() as usize;
        let mut txn_header = TransactionHeader::new(
            nonce,
            public_key_hex,
            from_addr,
            to_addr,
            weilpod_counter,
        );

        let signature = self.sign_execute_args(&wallet, &txn_header, &args)?;
        drop(wallet); // release lock before any I/O

        txn_header.set_signature(signature.as_str());
        let base_txn = BaseTransaction::new(txn_header);

        Ok((base_txn, signature, args))
    }

    /// Canonicalize and **sign** the execute payload using the provided wallet.
    ///
    /// - Builds a stable, sorted representation by converting the JSON payload
    ///   to a `BTreeMap` (`value_to_btreemap`) before serialization.
    /// - Signs the resulting bytes with `secp256k1` ECDSA via [`wallet::Wallet::sign`].
    fn sign_execute_args(
        &self,
        wallet: &Wallet,
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
                "should_hide_args": args.should_hide_args
            }
        });

        let json_payload_btreemap = value_to_btreemap(json_payload);
        let json_payload = serde_json::to_string(&json_payload_btreemap)?;
        wallet.sign(json_payload.as_bytes())
    }

    /// Common submission path for both normal and streaming executions.
    ///
    /// Accepts an API function (from [`PlatformApi`]) to perform the actual HTTP
    /// request. Concurrency is limited by the client's internal semaphore.
    async fn hit_api<R, T, F>(
        &self,
        signature: String,
        txn: &BaseTransaction,
        args: ExecuteArgs,
        is_non_blocking: bool,
        api: F,
    ) -> Result<R, anyhow::Error>
    where
        T: Future<Output = Result<R, anyhow::Error>>,
        F: Fn(SubmitTxnRequest, Client, bool) -> T,
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
                    should_hide_args: args.should_hide_args,
                },
            },
        };

        let _permit = self.client.semaphore.acquire().await.unwrap();

        let result = api(payload, self.client.http_client.clone(), is_non_blocking).await;

        drop(_permit);

        result
    }
}
