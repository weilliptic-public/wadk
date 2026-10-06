//! # Wallet primitives for WeilChain
//!
//! This module provides types to work with single or multiple WeilChain accounts:
//! - [`Account`] — a secp256k1 keypair with an associated address.
//! - [`Wallet`] — multi-account wallet loaded from a `wallet.wc` file, with
//!   account switching via [`SelectedAccount`].
//!
//! ## Address format
//! All account addresses are 72-char hex strings minted by the sentinel server
//! (embeds an obfuscated `weilpod_counter`). Addresses cannot be derived from
//! the private key alone.
//!
//! ## Usage
//! ```no_run
//! use weil_wallet::wallet::{Wallet, SelectedAccount};
//!
//! let mut wallet = Wallet::from_wallet_file("wallet.wc").unwrap();
//! wallet.set_index(&SelectedAccount::Derived(1)).unwrap();
//! ```

use crate::utils::hash_sha256;
use bip32::{ChildNumber, ExtendedPrivateKey};
use libsecp256k1::{Message, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path, str::FromStr};

const SENTINEL_HOST: &str = "https://sentinel.weilliptic.ai";

// ── Organization info ─────────────────────────────────────────────────────────

/// Organization membership linked via `wallet link-org`.
/// Used for v1 wallet files (backward compat) and as the public return type.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrgInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subgroup: Option<String>,
    #[serde(default)]
    pub purpose: String,
}

/// v2 org membership — no purpose (resolved at runtime).
#[derive(Debug, Clone, Deserialize)]
struct OrgMembershipV2 {
    org: String,
    #[serde(default)]
    subgroup: String,
}

// ── Wallet file format ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WalletFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(rename = "type")]
    file_type: String,
    xprv: String,
    #[allow(dead_code)]
    home_region: String,
    #[serde(default = "default_wallet_selected_account")]
    selected_account: WalletSelectedAccount,
    derived_accounts: Vec<WalletDerivedAccountEntry>,
    external_accounts: Vec<WalletExternalAccountEntry>,
    /// v1 format: single org
    #[serde(default)]
    org: Option<OrgInfo>,
    /// v2 format: multiple orgs (no purpose)
    #[serde(default)]
    orgs: Vec<OrgMembershipV2>,
    #[serde(default)]
    active_org: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WalletSelectedAccount {
    #[serde(alias = "type", alias = "account_type")]
    account_type: String,
    index: u32,
}

fn default_wallet_selected_account() -> WalletSelectedAccount {
    WalletSelectedAccount {
        account_type: "derived".to_string(),
        index: 0,
    }
}

#[derive(Debug, Deserialize)]
struct WalletDerivedAccountEntry {
    index: u32,
    public_key: String,
    account_address: String,
    #[serde(default)]
    orgs: Vec<OrgMembershipV2>,
    #[serde(default)]
    active_org: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WalletExternalAccountEntry {
    #[allow(dead_code)]
    index: u32,
    secret_key: String,
    account_address: String,
    #[serde(default)]
    orgs: Vec<OrgMembershipV2>,
    #[serde(default)]
    active_org: Option<usize>,
}

/// External S3 credentials for caller-owned buckets.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct S3Credentials {
    access_key_id: String,
    secret_access_key: String,
    bucket_name: String,
    region: String,
}

// ── SelectedAccount ───────────────────────────────────────────────────────────

/// Identifies which account in the wallet is currently active.
#[derive(Debug, Clone, Serialize)]
pub enum SelectedAccount {
    /// A BIP32 HD-derived account at the given index in `derived_accounts`.
    Derived(usize),
    /// An externally imported account at the given index in `added_accounts`.
    External(usize),
}

impl std::fmt::Display for SelectedAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectedAccount::Derived(i) => write!(f, "Derived Account {}", i),
            SelectedAccount::External(i) => write!(f, "External Account {}", i),
        }
    }
}

// ── Account ───────────────────────────────────────────────────────────────────

/// A single WeilChain account: secp256k1 keypair + address.
///
/// The address is a sentinel-minted 72-char hex string.
#[derive(Debug, Clone)]
pub struct Account {
    secret_key: Option<SecretKey>,
    public_key: PublicKey,
    account_address: String,
}

impl Account {
    fn from_secret_bytes_and_address(secret_bytes: &[u8], address: String) -> anyhow::Result<Self> {
        let secret_key = SecretKey::parse_slice(secret_bytes)?;
        let public_key = PublicKey::from_secret_key(&secret_key);
        Ok(Account {
            secret_key: Some(secret_key),
            public_key,
            account_address: address,
        })
    }

    /// Construct a [`Account`] from a hex-encoded secp256k1
    /// public key.
    ///
    /// `pk_hex` is decoded into raw SEC1 point bytes and parsed by `secp256k1`;                                                                      
    /// both the compressed (33-byte) and uncompressed (65-byte) encodings are                                                                        
    /// accepted. 
    ///
    /// # Why this exists
    /// Wallets loaded through [`Wallet::from_api_key`] have masked xprv: So
    /// we have only public keys and cant derive the secret keys from 
    /// masked xprv. Accounts built here therefore hold `secret_key: None` and
    /// cannot sign locally — [`Account::get_secret_key`] panics on them by
    /// design, and signing must be routed through [`Wallet::remote_sign`].
    fn from_public_key_hex(pk_hex: &str, address: String) -> anyhow::Result<Self> {
        let pk_bytes = hex::decode(pk_hex)
            .map_err(|e| anyhow::anyhow!("invalid public_key hex: {}", e))?;
        let public_key = PublicKey::parse_slice(&pk_bytes, None)
            .map_err(|e| anyhow::anyhow!("invalid public key: {}", e))?;
        Ok(Account {
            secret_key: None,
            public_key,
            account_address: address,
        })
    }

    /// Return the secp256k1 public key of this account.
    pub fn get_public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Return the sentinel-minted address of this account.
    pub fn get_address(&self) -> &str {
        &self.account_address
    }

    /// Returns the account's secp256k1 secret key.
    ///
    /// Panics for API-key accounts: they are public-only and sign remotely.
    pub fn get_secret_key(&self) -> &SecretKey {
        self.secret_key.as_ref().expect("secret key is missing")
    }

    /// Signs `buf` with ECDSA over secp256k1 (SHA-256 hashed). Returns the
    /// hex-encoded 64-byte compact signature.
    pub fn sign(&self, buf: &[u8]) -> anyhow::Result<String> {
        let digest = hash_sha256(buf);
        let msg = Message::parse_slice(&digest)?;
        let secret_key = self.get_secret_key();
        let (sig, _) = libsecp256k1::sign(&msg, secret_key);
        Ok(hex::encode(sig.serialize()))
    }
}

// ── Wallet ────────────────────────────────────────────────────────────────────

/// Multi-account secp256k1 wallet for the **WeilChain** platform.
///
/// Loaded from a `wallet.wc` file. Holds derived accounts (HD-derived from
/// the stored `xprv`) and external accounts (imported with their own secret
/// keys). All signing and address operations act on the currently selected
/// account.
#[derive(Clone)]
pub struct Wallet {
    derived_accounts: Vec<Account>,
    added_accounts: Vec<Account>,
    current_account_index: SelectedAccount,
    org: Option<OrgInfo>,
    wallet_file: Option<Value>,
}

impl Wallet {
    // ── Constructor ──────────────────────────────────────────────────────────

    /// Load a [`Wallet`] from a `wallet.wc` file.
    ///
    /// Derived account secret keys are re-derived from the stored `xprv`.
    /// External account secret keys are read directly from the file.
    /// The active account is set from the `selected_account` field (defaults
    /// to the first derived account when absent).
    pub fn from_wallet_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let mut fs = File::open(&path)?;
        let mut buf = String::new();
        fs.read_to_string(&mut buf)?;

        let wf: WalletFile = serde_json::from_str(&buf)?;
        if wf.file_type != "wallet" {
            anyhow::bail!("expected file type 'wallet', got '{}'", wf.file_type);
        }
        if wf.derived_accounts.is_empty() && wf.external_accounts.is_empty() {
            anyhow::bail!("wallet file contains no accounts");
        }

        let master = parse_xprv_with_path_fallback(&wf.xprv, &wf.derived_accounts)?;

        let mut derived_accounts: Vec<Account> = Vec::new();
        for entry in &wf.derived_accounts {
            let sk = derive_child_secret(&master, entry.index as usize)?;
            derived_accounts.push(Account::from_secret_bytes_and_address(
                &sk.serialize(),
                entry.account_address.clone(),
            )?);
        }

        let mut added_accounts: Vec<Account> = Vec::new();
        for entry in &wf.external_accounts {
            let bytes = hex::decode(&entry.secret_key)?;
            added_accounts.push(Account::from_secret_bytes_and_address(
                &bytes,
                entry.account_address.clone(),
            )?);
        }

        let current_account_index = match wf.selected_account.account_type.as_str() {
            "external" => {
                let idx = wf.selected_account.index as usize;
                if idx >= added_accounts.len() {
                    anyhow::bail!(
                        "selected external account index {} out of bounds (have {})",
                        idx,
                        added_accounts.len()
                    );
                }
                SelectedAccount::External(idx)
            }
            _ => {
                let idx = wf.selected_account.index as usize;
                if idx >= derived_accounts.len() {
                    anyhow::bail!(
                        "selected derived account index {} out of bounds (have {})",
                        idx,
                        derived_accounts.len()
                    );
                }
                SelectedAccount::Derived(idx)
            }
        };

        // Resolve org from the selected account's per-account orgs[],
        // then fall back to top-level orgs[], then v1 org field.
        let selected_idx = wf.selected_account.index as usize;
        let selected_type = wf.selected_account.account_type.as_str();

        let (account_orgs, account_active_org) = match selected_type {
            "external" => {
                let entry = wf.external_accounts.get(selected_idx);
                (
                    entry.map(|e| &e.orgs).cloned().unwrap_or_default(),
                    entry.and_then(|e| e.active_org),
                )
            }
            _ => {
                let entry = wf.derived_accounts.get(selected_idx);
                (
                    entry.map(|e| &e.orgs).cloned().unwrap_or_default(),
                    entry.and_then(|e| e.active_org),
                )
            }
        };

        let org = if !account_orgs.is_empty() {
            // Per-account v2 orgs (preferred)
            let idx = account_active_org.unwrap_or(0);
            account_orgs.get(idx).map(|m| OrgInfo {
                name: m.org.clone(),
                subgroup: if m.subgroup.is_empty() {
                    None
                } else {
                    Some(m.subgroup.clone())
                },
                purpose: String::new(),
            })
        } else if !wf.orgs.is_empty() {
            // Top-level v2 orgs (fallback for CLI wallet exports)
            let idx = wf.active_org.unwrap_or(0);
            wf.orgs.get(idx).map(|m| OrgInfo {
                name: m.org.clone(),
                subgroup: if m.subgroup.is_empty() {
                    None
                } else {
                    Some(m.subgroup.clone())
                },
                purpose: String::new(),
            })
        } else {
            // v1 single org field
            wf.org
        };

        Ok(Self {
            derived_accounts,
            added_accounts,
            current_account_index,
            org,
            wallet_file: None,
        })
    }

    /// Load a [`Wallet`] using an Agent Registry API key.
    ///
    /// Fetches the wallet JSON from the sentinel `/get_agent_wallet` endpoint,
    /// then parses and constructs the wallet.
    pub async fn from_api_key(
        api_key: &str,
        creds: Option<S3Credentials>,
        sentinel_host: Option<String>,
    ) -> anyhow::Result<Self> {
        let wallet_json = Self::get_agent_wallet(api_key, creds, false, sentinel_host).await?;
        let wf: Value = serde_json::from_str(&wallet_json)
            .map_err(|e| anyhow::anyhow!("failed to parse wallet JSON from sentinel: {}", e))?;

        let derived_account_values: Vec<&Value> = wf
            .get("derived_accounts")
            .and_then(Value::as_array)
            .map(|v| v.iter().collect())
            .unwrap_or_default();

        let derived_accounts: Vec<Account> = derived_account_values
            .iter()
            .map(|entry| {
                let addr = entry.get("account_address")
                    .and_then(Value::as_str).unwrap_or("").to_string();
                let pk = entry.get("public_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("derived account missing public_key"))?;
                Account::from_public_key_hex(pk, addr)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let added_accounts: Vec<Account> = wf.get("external_accounts")
            .and_then(Value::as_array)
            .map(|v| v.iter())
            .into_iter()
            .flatten()
            .map(|entry| {
                let addr = entry.get("account_address")
                    .and_then(Value::as_str).unwrap_or("").to_string();
                let sk = entry.get("secret_key")
                    .and_then(Value::as_str).unwrap_or("").to_string();
                let secret_bytes = hex::decode(&sk)
                    .map_err(|e| anyhow::anyhow!("invalid external secret_key hex: {}", e))?;
                Account::from_secret_bytes_and_address(&secret_bytes, addr)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let orginfo = derived_account_values.first().and_then(|v| {
            let idx = v.get("active_org").and_then(Value::as_u64).unwrap_or(0) as usize;
            let orgs = v.get("orgs").and_then(Value::as_array)?;
            let m = orgs.get(idx)?;

            Some(OrgInfo {
                name: m.get("org")?.as_str()?.to_string(),
                subgroup: m
                    .get("subgroup")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from),
                purpose: String::new(),
            })
        });

        let sel: WalletSelectedAccount = wf.get("selected_account")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(WalletSelectedAccount {
                account_type: "derived".into(),
                index: 0,
            });

        let idx = sel.index as usize;
        let current_account_index = match sel.account_type.as_str() {
            "external" if idx < added_accounts.len() => SelectedAccount::External(idx),
            "derived" if idx < derived_accounts.len() => SelectedAccount::Derived(idx),
            _ => SelectedAccount::Derived(0),
        };

        Ok(Self {
            derived_accounts,
            added_accounts,
            current_account_index,
            org: orginfo,
            wallet_file: Some(wf),
        })
    }

    /// Fetch the wallet JSON for an Agent Registry API key from sentinel.
    ///
    /// POSTs `{ "api_key": ..., "credentials": ... }` to `/get_agent_wallet`.
    /// The endpoint returns either a JSON string (the wallet file contents) or
    /// an error object. Non-JSON responses are treated as error messages.
    async fn get_agent_wallet(
        api_key: &str,
        creds: Option<S3Credentials>,
        unmasked: bool,
        sentinel_host: Option<String>,
    ) -> anyhow::Result<String> {
        let mut payload = serde_json::json!({ "api_key": api_key });
        if let Some(c) = creds {
            payload["credentials"] = serde_json::to_value(c)?;
        }
        payload["unmasked"] = serde_json::to_value(unmasked)?;

        let sentinel_host = sentinel_host.unwrap_or_else(|| SENTINEL_HOST.to_string());
        let url = format!("{}/get_agent_wallet", sentinel_host);

        let client = reqwest::Client::builder().build()?;

        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("agent wallet lookup request failed: {}", e))?;

        let status = resp.status();
        let body = resp.text().await
            .map_err(|e| anyhow::anyhow!("failed to read agent wallet response: {}", e))?;

        let result: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) if body.trim().is_empty() => anyhow::bail!("agent wallet not found for API key"),
            Err(_) => anyhow::bail!("agent wallet lookup failed: {}", body.trim()),
        };

        if !status.is_success() {
            anyhow::bail!("agent wallet lookup failed: HTTP {} {}", status, result);
        }

        let wallet_json = result.as_str()
            .ok_or_else(|| anyhow::anyhow!("agent wallet lookup failed: unexpected response: {:?}", result))?
            .trim();

        if wallet_json.is_empty() {
            anyhow::bail!("agent wallet not found for API key");
        }

        Ok(wallet_json.to_string())
    }
    // ── Account management ────────────────────────────────────────────────────

    /// Switch the active account.
    ///
    /// Returns an error if the index is out of bounds for the target list.
    pub fn set_index(&mut self, selected: &SelectedAccount) -> anyhow::Result<()> {
        match selected {
            SelectedAccount::Derived(index) => {
                if *index < self.derived_accounts.len() {
                    self.current_account_index = selected.clone();
                    Ok(())
                } else {
                    anyhow::bail!(
                        "derived account index {} out of bounds (have {} derived account(s))",
                        index,
                        self.derived_accounts.len()
                    )
                }
            }
            SelectedAccount::External(index) => {
                if *index < self.added_accounts.len() {
                    self.current_account_index = selected.clone();
                    Ok(())
                } else {
                    anyhow::bail!(
                        "external account index {} out of bounds (have {} external account(s))",
                        index,
                        self.added_accounts.len()
                    )
                }
            }
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Return the secp256k1 public key of the currently selected account.
    pub fn get_public_key(&self) -> PublicKey {
        self.current_account().get_public_key()
    }

    /// Return the address of the currently selected account.
    pub fn get_address(&self) -> &str {
        self.current_account().get_address()
    }

    /// Return the number of derived accounts.
    pub fn derived_account_count(&self) -> usize {
        self.derived_accounts.len()
    }

    /// Return the number of externally added accounts.
    pub fn external_account_count(&self) -> usize {
        self.added_accounts.len()
    }

    /// Return the currently selected account index.
    pub fn current_account_index(&self) -> &SelectedAccount {
        &self.current_account_index
    }

    /// Return the org info if this wallet has a linked organization.
    pub fn org(&self) -> Option<&OrgInfo> {
        self.org.as_ref()
    }

    /// Sign `buf` with the currently selected account using **ECDSA over secp256k1**.
    ///
    /// The message is first hashed with **SHA-256**, then signed. Returns the
    /// hex-encoded compact (64-byte) signature.
    pub fn sign(&self, buf: &[u8]) -> anyhow::Result<String> {
        self.current_account().sign(buf)
    }

    /// Signs a canonical JSON payload via Sentinel's `/sign_payload` endpoint.
    ///
    /// Used by API-key wallets, which hold no local secret key. Sends the
    /// payload, the masked wallet file, and optional S3 credentials; returns
    /// the hex signature. Errors if the wallet was not created from an API key,
    /// or on a non-2xx / signature-less response.
    pub(crate) async fn remote_sign(
        &self,
        buf: &Value,
        sentinel_host: &str,
        credentials: Option<S3Credentials>,
        client: &reqwest::Client,
    ) -> anyhow::Result<String> {
        let url = format!("{}/sign_payload", sentinel_host);
        let wallet_file = self.wallet_file.as_ref()
            .ok_or_else(|| anyhow::anyhow!("remote signing requires an API-key wallet"))?;
        let body = serde_json::json!({ "payload": buf, "wallet": wallet_file, "credentials": credentials });

        let resp = client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("sign_payload failed: HTTP {} {}", status, text);
        }
        let data: serde_json::Value = resp.json().await?;
        let signed_payload = data
            .get("signature")
            .or_else(|| data.get("Ok"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "sign_payload failed: {}",
                    data.get("Err")
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| data.to_string())
                )
            })?;
        Ok(signed_payload.to_string())
    }

    fn current_account(&self) -> &Account {
        match self.current_account_index {
            SelectedAccount::Derived(i) => &self.derived_accounts[i],
            SelectedAccount::External(i) => &self.added_accounts[i],
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Derive the libsecp256k1 `SecretKey` for child `index` from `master`.
fn derive_child_secret(
    master: &ExtendedPrivateKey<bip32::secp256k1::ecdsa::SigningKey>,
    index: usize,
) -> anyhow::Result<SecretKey> {
    let child_num = ChildNumber::new(index as u32, false)
        .map_err(|e| anyhow::anyhow!("invalid child number {}: {}", index, e))?;
    let child_xprv = master
        .derive_child(child_num)
        .map_err(|e| anyhow::anyhow!("derive_child({}) failed: {}", index, e))?;
    SecretKey::parse_slice(&child_xprv.to_bytes())
        .map_err(|e| anyhow::anyhow!("parse derived secret key: {}", e))
}

/// Parse the `xprv` string from a wallet file, applying the BIP44 path
/// `m/44'/9345'/0'/0` when the stored key is a root key rather than an
/// already-derived account-level key.
///
/// Detection: derive child 0 and compare the resulting compressed public key
/// against the first derived account's stored `public_key`. If they match the
/// xprv is already at the account level; otherwise traverse the full path first.
fn parse_xprv_with_path_fallback(
    xprv_str: &str,
    derived_accounts: &[WalletDerivedAccountEntry],
) -> anyhow::Result<ExtendedPrivateKey<bip32::secp256k1::ecdsa::SigningKey>> {
    let parsed = ExtendedPrivateKey::from_str(xprv_str)
        .map_err(|e| anyhow::anyhow!("could not parse xprv: {}", e))?;

    if let Some(first) = derived_accounts.first() {
        let sk_direct = derive_child_secret(&parsed, first.index as usize)?;
        let pk_direct = PublicKey::from_secret_key(&sk_direct);
        let pk_direct_hex = hex::encode(pk_direct.serialize_compressed());

        if pk_direct_hex == first.public_key {
            return Ok(parsed);
        }

        // Root xprv — traverse m/44'/9345'/0'/0 first.
        let path: [ChildNumber; 4] = [
            ChildNumber::new(44, true).unwrap(),
            ChildNumber::new(9345, true).unwrap(),
            ChildNumber::new(0, true).unwrap(),
            ChildNumber::new(0, false).unwrap(),
        ];
        let mut key = parsed;
        for child in &path {
            key = key
                .derive_child(*child)
                .map_err(|e| anyhow::anyhow!("BIP44 path derivation failed: {}", e))?;
        }
        Ok(key)
    } else {
        Ok(parsed)
    }
}
