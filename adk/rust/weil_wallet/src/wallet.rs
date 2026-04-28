//! # Wallet primitives for WeilChain
//!
//! This module provides lightweight types to work with a WeilChain "account":
//! - [`PrivateKey`] — loads a hex-encoded private key from disk.
//! - [`Wallet`] — constructs a signing account from a private key and exposes
//!   its secp256k1 public key and signing capability.
//!
//! ## Notes & Security
//! - Private keys are expected to be **hex-encoded** (even length, 0–9a–f/A–F).
//! - Keys are held in memory as plain bytes/strings; avoid logging them.
//! - Signing uses **secp256k1** with ECDSA over the SHA-256 digest of the input.

use crate::utils::{get_address_from_public_key, hash_sha256};
use bip32::{secp256k1::ecdsa::SigningKey, ChildNumber, ExtendedPrivateKey, Prefix};
use libsecp256k1::{Message, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::Path, str::FromStr};

/// Represents the private key associated with your account.
///
/// This should match the key used by the official **Weilliptic** CLI or browser wallet
/// if you want the exact same on-chain identity.
#[derive(Debug, Clone)]
pub struct PrivateKey(String);

impl PrivateKey {
    /// Construct a [`PrivateKey`] by reading a **hex-encoded** key from a file.
    ///
    /// The file must contain a non-empty hexadecimal string (even-length). Any
    /// surrounding whitespace is trimmed before validation.
    ///
    /// # Errors
    /// - If the file cannot be opened or read.
    /// - If the file is empty after trimming.
    /// - If the contents are not valid hexadecimal.
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let mut fs = File::open(path)?;
        let mut buf = String::new();

        fs.read_to_string(&mut buf)?;

        if buf.trim().is_empty() {
            return Err(anyhow::Error::msg("private key file is empty"));
        }

        // Ensure the private key string is valid hexadecimal
        let buf_trimmed = buf.trim();

        if buf_trimmed.len() % 2 != 0 || !buf_trimmed.chars().all(|c| c.is_digit(16)) {
            return Err(anyhow::Error::msg(
                "private key is not a valid hexadecimal string",
            ));
        }

        Ok(PrivateKey(buf_trimmed.to_string()))
    }

    /// Return the inner hex string.
    pub fn hex(&self) -> &str {
        &self.0
    }
}


// ── Account export format ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AccountExportFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(rename = "type")]
    export_type: String,
    account: AccountExportEntry,
}

#[derive(Debug, Deserialize)]
struct AccountExportEntry {
    secret_key: String,
    account_address: String,
}

// ── Wallet file format (wallet.wc) ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WalletFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(rename = "type")]
    file_type: String,
    xprv: String,
    #[allow(dead_code)]
    home_region: Option<String>,
    #[serde(default = "default_wallet_selected_account")]
    selected_account: WalletSelectedAccount,
    #[serde(default)]
    derived_accounts: Vec<WalletDerivedAccountEntry>,
    #[serde(default)]
    external_accounts: Vec<WalletExternalAccountEntry>,
}

#[derive(Debug, Deserialize)]
struct WalletSelectedAccount {
    #[serde(rename = "type")]
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
}

#[derive(Debug, Deserialize)]
struct WalletExternalAccountEntry {
    #[allow(dead_code)]
    index: u32,
    secret_key: String,
    account_address: String,
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
/// For derived accounts the address is `hex(SHA-256(compressed_pubkey))`.
/// For external accounts it is the sentinel-minted 72-char hex string.
#[derive(Debug, Clone)]
pub struct Account {
    secret_key: SecretKey,
    public_key: PublicKey,
    account_address: String,
}

impl Account {
    /// Build from raw private key bytes + a pre-computed address string.
    fn from_secret_bytes_and_address(secret_bytes: &[u8], address: String) -> anyhow::Result<Self> {
        let secret_key = SecretKey::parse_slice(secret_bytes)?;
        let public_key = PublicKey::from_secret_key(&secret_key);
        Ok(Account { secret_key, public_key, account_address: address })
    }

    /// Build from a hex-encoded [`PrivateKey`] + a pre-minted sentinel address.
    fn from_private_key_and_address(key: &PrivateKey, address: String) -> anyhow::Result<Self> {
        let bytes = hex::decode(key.hex())?;
        Self::from_secret_bytes_and_address(&bytes, address)
    }

    pub fn get_public_key(&self) -> PublicKey {
        self.public_key
    }

    pub fn get_address(&self) -> &str {
        &self.account_address
    }

    pub fn get_secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn sign(&self, buf: &[u8]) -> anyhow::Result<String> {
        let digest = hash_sha256(buf);
        let msg = Message::parse_slice(&digest)?;
        let (sig, _) = libsecp256k1::sign(&msg, &self.secret_key);
        Ok(hex::encode(sig.serialize()))
    }
}

// ── Wallet ────────────────────────────────────────────────────────────────────

/// Multi-account secp256k1 wallet for the **WeilChain** platform.
///
/// Holds two independent account lists:
/// - `derived_accounts` — BIP32 HD-derived from `master_secret_key`.
/// - `added_accounts`   — externally imported (from sentinel export files or raw key bytes).
///
/// All signing and address operations act on the currently selected account.
#[derive(Clone)]
pub struct Wallet {
    /// Present when the wallet was created with a master extended private key.
    master_secret_key: Option<ExtendedPrivateKey<SigningKey>>,
    derived_accounts: Vec<Account>,
    added_accounts: Vec<Account>,
    current_account_index: SelectedAccount,
}

impl Wallet {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Create a wallet by deriving `children` accounts from an already-parsed
    /// BIP32 extended private key. Derives at path indices `0..children`.
    ///
    /// The first derived account (`Derived(0)`) is selected by default.
    pub fn from_master_key(
        master_secret_key: ExtendedPrivateKey<SigningKey>,
        children: usize,
    ) -> anyhow::Result<Self> {
        let mut derived_accounts = Vec::with_capacity(children);

        for i in 0..children {
            let account = derive_child_account(&master_secret_key, i)?;
            derived_accounts.push(account);
        }

        if derived_accounts.is_empty() {
            anyhow::bail!("must derive at least one account (children must be > 0)");
        }

        Ok(Self {
            master_secret_key: Some(master_secret_key),
            derived_accounts,
            added_accounts: Vec::new(),
            current_account_index: SelectedAccount::Derived(0),
        })
    }

    /// Create a wallet by parsing a BIP32 XPRV string and deriving `children` accounts.
    pub fn from_master_key_str(xprv: &str, children: usize) -> anyhow::Result<Self> {
        let master = ExtendedPrivateKey::from_str(xprv)
            .map_err(|e| anyhow::anyhow!("failed to parse extended private key: {}", e))?;
        Self::from_master_key(master, children)
    }

    /// Create a [`Wallet`] from a raw private key and a pre-minted 72-char
    /// account address. No sentinel connection required.
    pub fn from_private_key_and_address(
        private_key: &PrivateKey,
        account_address: String,
    ) -> anyhow::Result<Self> {
        let account = Account::from_private_key_and_address(private_key, account_address)?;
        Ok(Self {
            master_secret_key: None,
            derived_accounts: Vec::new(),
            added_accounts: vec![account],
            current_account_index: SelectedAccount::External(0),
        })
    }

    /// Create a [`Wallet`] from a single-account export JSON file.
    /// No sentinel connection required.
    ///
    /// The file is produced by the CLI's `wallet export-account` command or
    /// the browser wallet's export feature.
    pub fn from_account_export_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let account = account_from_export_file(path)?;
        Ok(Self {
            master_secret_key: None,
            derived_accounts: Vec::new(),
            added_accounts: vec![account],
            current_account_index: SelectedAccount::External(0),
        })
    }

    /// Load a [`Wallet`] from a `wallet.wc` file.
    ///
    /// This matches the multi-account wallet export format used by the Weil browser/CLI.
    /// Derived account secret keys are re-derived from the stored `xprv`. External
    /// account secret keys are read directly from the file.
    ///
    /// The active account is set from the `selected_account` field (defaults to
    /// the first derived account when absent).
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

        Ok(Self {
            master_secret_key: Some(master),
            derived_accounts,
            added_accounts,
            current_account_index,
        })
    }

    // ── Account management ────────────────────────────────────────────────────

    /// Derive the next child account from the master key and append it to `derived_accounts`.
    ///
    /// Prints the xpub and address of the new account (mirroring the reference implementation).
    /// Returns an error if the wallet has no master key.
    pub fn derive_new_account(&mut self) -> anyhow::Result<()> {
        let master = self
            .master_secret_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("wallet has no master key; cannot derive accounts"))?;

        let index = self.derived_accounts.len();
        let account = derive_child_account(master, index)?;

        // Reconstruct xpub for the new child to print (mirrors reference behaviour).
        let account_number = ChildNumber::new(index as u32, false)
            .map_err(|e| anyhow::anyhow!("invalid child number: {}", e))?;
        let child_xprv = master
            .derive_child(account_number)
            .map_err(|e| anyhow::anyhow!("HD derivation failed: {}", e))?;
        println!(
            "xpub: {}\naddress: {}",
            child_xprv.public_key().to_string(Prefix::XPUB),
            account.get_address()
        );

        self.derived_accounts.push(account);
        Ok(())
    }

    /// Add an externally owned account from raw private key bytes.
    ///
    /// Address is derived as `hex(SHA-256(compressed_pubkey))`.
    pub fn add_new_account(&mut self, p_key: &[u8]) -> anyhow::Result<()> {
        let secret_key = SecretKey::parse_slice(p_key)?;
        let public_key = PublicKey::from_secret_key(&secret_key);
        let address = (*get_address_from_public_key(&public_key)).clone();
        self.added_accounts.push(Account {
            secret_key,
            public_key,
            account_address: address,
        });
        Ok(())
    }

    /// Add an externally owned account from a sentinel account export JSON file.
    ///
    /// The new account is appended; the selected account does not change.
    pub fn add_account_from_export_file<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let account = account_from_export_file(path)?;
        self.added_accounts.push(account);
        Ok(())
    }

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

    /// Sign `buf` with the currently selected account using **ECDSA over secp256k1**.
    ///
    /// The message is first hashed with **SHA-256**, then signed. Returns the
    /// hex-encoded compact (64-byte) signature.
    pub fn sign(&self, buf: &[u8]) -> anyhow::Result<String> {
        self.current_account().sign(buf)
    }

    // ── Serialization (mirrors reference `to_string` / `from_string`) ─────────

    /// Serialize the wallet to a newline-delimited string:
    /// - Line 1: master extended private key (XPRV format), or empty if none.
    /// - Line 2: number of derived accounts.
    /// - Lines 3+: hex-encoded secret keys of externally added accounts.
    pub fn to_wallet_string(&self) -> anyhow::Result<String> {
        let mut parts: Vec<String> = Vec::new();

        parts.push(match &self.master_secret_key {
            Some(k) => k.to_string(Prefix::XPRV).to_string(),
            None => String::new(),
        });
        parts.push(self.derived_accounts.len().to_string());

        for acc in &self.added_accounts {
            parts.push(hex::encode(acc.secret_key.serialize()));
        }

        Ok(parts.join("\n"))
    }

    /// Deserialize a wallet previously produced by [`Self::to_wallet_string`].
    pub fn from_wallet_string(buf: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = buf.split('\n').collect();
        if parts.len() < 2 {
            anyhow::bail!("invalid wallet string: too few lines");
        }

        let master_str = parts[0];
        let da_len: usize = parts[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("could not parse derived account count"))?;

        let mut added_accounts: Vec<Account> = Vec::new();
        for part in &parts[2..] {
            if part.is_empty() {
                continue;
            }
            let sk_bytes = hex::decode(part)
                .map_err(|_| anyhow::anyhow!("could not decode external account key"))?;
            let secret_key = SecretKey::parse_slice(&sk_bytes)
                .map_err(|e| anyhow::anyhow!("invalid external account key: {}", e))?;
            let public_key = PublicKey::from_secret_key(&secret_key);
            let address = (*get_address_from_public_key(&public_key)).clone();
            added_accounts.push(Account { secret_key, public_key, account_address: address });
        }

        if master_str.is_empty() {
            // External-only wallet
            if added_accounts.is_empty() {
                anyhow::bail!("wallet string has no master key and no external accounts");
            }
            return Ok(Self {
                master_secret_key: None,
                derived_accounts: Vec::new(),
                added_accounts,
                current_account_index: SelectedAccount::External(0),
            });
        }

        let master = ExtendedPrivateKey::from_str(master_str)
            .map_err(|e| anyhow::anyhow!("failed to parse extended private key: {}", e))?;

        let mut wallet = Self::from_master_key(master, da_len)?;
        wallet.added_accounts = added_accounts;
        Ok(wallet)
    }

    fn current_account(&self) -> &Account {
        match self.current_account_index {
            SelectedAccount::Derived(i) => &self.derived_accounts[i],
            SelectedAccount::External(i) => &self.added_accounts[i],
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Derive the child account at `index` from `master`, using non-hardened derivation.
fn derive_child_account(
    master: &ExtendedPrivateKey<SigningKey>,
    index: usize,
) -> anyhow::Result<Account> {
    let account_number = ChildNumber::new(index as u32, false)
        .map_err(|e| anyhow::anyhow!("invalid child number {}: {}", index, e))?;
    let child_xprv = master
        .derive_child(account_number)
        .map_err(|e| anyhow::anyhow!("HD derivation at index {} failed: {}", index, e))?;

    let secret_bytes = child_xprv.to_bytes();
    let secret_key = SecretKey::parse_slice(&secret_bytes)
        .map_err(|e| anyhow::anyhow!("invalid derived key at index {}: {}", index, e))?;
    let public_key = PublicKey::from_secret_key(&secret_key);
    let address = (*get_address_from_public_key(&public_key)).clone();

    Ok(Account { secret_key, public_key, account_address: address })
}

/// Derive the libsecp256k1 `SecretKey` for child `index` from `master`.
fn derive_child_secret(master: &ExtendedPrivateKey<SigningKey>, index: usize) -> anyhow::Result<SecretKey> {
    let child_num = ChildNumber::new(index as u32, false)
        .map_err(|e| anyhow::anyhow!("invalid child number {}: {}", index, e))?;
    let child_xprv = master
        .derive_child(child_num)
        .map_err(|e| anyhow::anyhow!("derive_child({}) failed: {}", index, e))?;
    SecretKey::parse_slice(&child_xprv.to_bytes())
        .map_err(|e| anyhow::anyhow!("parse derived secret key: {}", e))
}

fn account_from_export_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Account> {
    let mut fs = File::open(&path)?;
    let mut buf = String::new();
    fs.read_to_string(&mut buf)?;

    let export: AccountExportFile = serde_json::from_str(&buf)?;
    if export.export_type != "account" {
        anyhow::bail!(
            "expected export type 'account', got '{}'",
            export.export_type
        );
    }

    let key = PrivateKey(export.account.secret_key);
    Account::from_private_key_and_address(&key, export.account.account_address)
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
) -> anyhow::Result<ExtendedPrivateKey<SigningKey>> {
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
