//! # `WeilType`: Persistable value trait and blanket impls
//!
//! This module defines [`WeilType`], the core marker/constraint used for values that
//! are eligible to be **persisted efficiently** (e.g., contract state) within the
//! Weil runtime. It requires `Serialize + DeserializeOwned` to guarantee stable,
//! unambiguous (de)serialization across the WASM boundary and storage layers.
//!
//! In addition to the trait itself, this file provides blanket implementations for:
//! - Primitive numeric and scalar types (`u8`, `i64`, `bool`, `char`, etc.)
//! - Common standard containers (`Vec`, `BTreeMap`, `BTreeSet`, `Option`, `Box`)
//! - Tuples up to arity 5
//! - The unit type `()`
//! - Fixed-size byte array `[u8; 32]`
//! - The configuration wrapper [`Secrets<T>`]
//!
//! ## Note on storage behavior
//! Even though these types are *bounded* by [`WeilType`], **basic scalar types are
//! persisted by value** (not shallowly). Containers and composites are persisted by
//! recursively persisting their inner [`WeilType`] members.

use serde::{de::DeserializeOwned, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::config::Secrets;

/// Primary trait for any object that must be persisted efficiently (e.g., contract state).
///
/// Types implementing this trait must be serializable and deserializable without
/// lifetime ties (hence `DeserializeOwned`). Implementors should ensure forward-
/// compatible encoding when schema evolution is expected.
///
/// In most cases you will rely on the provided blanket implementations rather than
/// implementing this trait manually.
pub trait WeilType: Serialize + DeserializeOwned {}

// NOTE: Even though basic types are bounded by WeilType
// they are persisted by value and not shallowly.

// --- Primitive scalars ---
impl WeilType for u8 {}
impl WeilType for u16 {}
impl WeilType for u32 {}
impl WeilType for u64 {}
impl WeilType for u128 {}
impl WeilType for usize {}
impl WeilType for i8 {}
impl WeilType for i16 {}
impl WeilType for i32 {}
impl WeilType for i64 {}
impl WeilType for i128 {}
impl WeilType for isize {}
impl WeilType for f32 {}
impl WeilType for f64 {}
impl WeilType for bool {}
impl WeilType for char {}
impl WeilType for String {}

// --- Standard containers & composites ---
impl<T: WeilType> WeilType for Vec<T> {}
impl<K: WeilType + Ord, V: WeilType> WeilType for BTreeMap<K, V> {}
impl<T: WeilType + Ord> WeilType for BTreeSet<T> {}
impl<T: WeilType> WeilType for Box<T> {}
impl<T: WeilType> WeilType for Option<T> {}
impl WeilType for () {}
impl<T: WeilType> WeilType for Secrets<T> {}

// --- Tuples (up to arity 5) ---
impl<A: WeilType, B: WeilType> WeilType for (A, B) {}
impl<A: WeilType, B: WeilType, C: WeilType> WeilType for (A, B, C) {}
impl<A: WeilType, B: WeilType, C: WeilType, D: WeilType> WeilType for (A, B, C, D) {}
impl<A: WeilType, B: WeilType, C: WeilType, D: WeilType, E: WeilType> WeilType for (A, B, C, D, E) {}

// --- Fixed-size byte arrays ---
impl WeilType for [u8; 32] {}
