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

use crate::utils::hash_sha256;
use libsecp256k1::{Message, PublicKey, SecretKey};
use std::{fs::File, io::Read, path::Path};

/// Represents the private key associated with your account.
///
/// This should match the key used by the official **Weilliptic** CLI or browser wallet
/// if you want the exact same on-chain identity.
#[derive(Debug)]
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
}

/// Internal account material: secp256k1 secret & public keys.
#[derive(Debug)]
struct Account {
    secret_key: SecretKey,
    public_key: PublicKey,
}

/// A simple secp256k1-backed wallet for the **WeilChain** platform.
///
/// Use the same private key as your official Weilliptic CLI / browser wallet to
/// control the same on-chain account from this SDK.
#[derive(Debug)]
pub struct Wallet {
    account: Account,
}

impl Wallet {
    /// Create a new [`Wallet`] from a previously loaded [`PrivateKey`].
    ///
    /// Decodes the hex key, parses a secp256k1 [`SecretKey`], and derives the
    /// corresponding [`PublicKey`].
    ///
    /// # Errors
    /// - If the private key is not valid hex.
    /// - If the key material cannot be parsed as a secp256k1 secret key.
    pub fn new(private_key: PrivateKey) -> anyhow::Result<Self> {
        let secret_key_bytes = hex::decode(&private_key.0)?;
        let secret_key = SecretKey::parse_slice(&secret_key_bytes)?;

        let account = Account {
            secret_key,
            public_key: PublicKey::from_secret_key(&secret_key),
        };

        Ok(Wallet { account })
    }

    /// Return a reference to the account's secp256k1 secret key.
    ///
    /// > ⚠️ **Security:** Handle with care. Avoid logging or exposing this value.
    pub fn secrete_key(&self) -> &SecretKey {
        &self.account.secret_key
    }

    /// Return the account's secp256k1 public key.
    pub fn get_public_key(&self) -> PublicKey {
        self.account.public_key
    }

    /// Sign `buf` with the account's secret key using **ECDSA over secp256k1**.
    ///
    /// The message is first hashed with **SHA-256**, then signed. The returned
    /// signature is the hex-encoded compact (64-byte) representation.
    ///
    /// # Errors
    /// - If the message digest cannot be parsed into a secp256k1 [`Message`]
    ///   (should not occur for 32-byte SHA-256 digests).
    pub fn sign(&self, buf: &[u8]) -> anyhow::Result<String> {
        let digest = hash_sha256(buf);
        let secp_message = Message::parse_slice(&digest)?;

        let sig_result = libsecp256k1::sign(&secp_message, self.secrete_key());
        let signature = sig_result.0;

        Ok(hex::encode(&signature.serialize()))
    }
}
