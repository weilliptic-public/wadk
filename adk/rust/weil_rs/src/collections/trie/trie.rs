use super::map::WeilTriePrefixMap;
use crate::{
    collections::{WeilCollection, WeilId},
    runtime::Memory,
    traits::WeilType,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;

/// A shallow light weight lazy data-structure similar to `WeilMap<String, T>` with extra constraint
/// of key to be of type `String` in favour of supporting prefixed queries.
/// See the methods for more info.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilTrieMap<T> {
    state_id: WeilId,
    phantom: PhantomData<T>,
}

impl<T> WeilTrieMap<T> {
    /// Constructs a new empty `WeilTrieMap<T>`.
    pub fn new(id: WeilId) -> Self {
        WeilTrieMap {
            state_id: id,
            phantom: PhantomData,
        }
    }
}

impl<T> WeilTrieMap<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Behave similar to `WeilMap<String, T>`
    pub fn insert(&mut self, key: String, val: T) -> Result<(), String> {
        self.do_insert(None, key, val)
    }

    pub fn insert_with_suffix(
        &mut self,
        suffix: String,
        key: String,
        val: T,
    ) -> Result<(), String> {
        self.do_insert(Some(suffix), key, val)
    }

    fn do_insert(&mut self, suffix: Option<String>, key: String, val: T) -> Result<(), String> {
        let storage_key = self.state_tree_key(&key);
        match suffix {
            Some(suffix) => {
                Memory::write_collection_with_suffix(storage_key, val, suffix.as_str())
            }
            None => Memory::write_collection(storage_key, val),
        }
    }

    /// Behave similar to `WeilMap<String, T>`
    pub fn get(&self, key: &str) -> Option<T> {
        self.do_get(None, key)
    }

    pub fn get_with_suffix(&self, suffix: String, key: &str) -> Option<T> {
        self.do_get(Some(suffix), key)
    }

    fn do_get(&self, suffix: Option<String>, key: &str) -> Option<T> {
        let storage_key = self.state_tree_key(key);
        match suffix {
            Some(suffix) => Memory::read_collection_with_suffix(storage_key, suffix.as_str()),
            None => Memory::read_collection(storage_key),
        }
    }

    /// Behave similar to `WeilMap<String, T>`
    pub fn remove(&self, key: &str) -> Option<T> {
        self.do_remove(None, key)
    }

    pub fn remove_with_suffix(&self, suffix: String, key: &str) -> Option<T> {
        self.do_remove(Some(suffix), key)
    }

    fn do_remove(&self, suffix: Option<String>, key: &str) -> Option<T> {
        let storage_key = self.state_tree_key(key);
        match suffix {
            Some(suffix) => Memory::delete_collection_with_suffix(storage_key, suffix.as_str()),
            None => Memory::delete_collection(storage_key),
        }
    }

    /// Returns all the values in pair with their corrosponding keys which share the provided prefix.
    pub fn get_with_prefix(&self, prefix: &str) -> Option<WeilTriePrefixMap<T>> {
        self.do_get_with_prefix_and_suffix(None, prefix)
    }

    pub fn get_with_prefix_and_suffix(
        &self,
        suffix: String,
        prefix: &str,
    ) -> Option<WeilTriePrefixMap<T>> {
        self.do_get_with_prefix_and_suffix(Some(suffix), prefix)
    }

    fn do_get_with_prefix_and_suffix(
        &self,
        suffix: Option<String>,
        prefix: &str,
    ) -> Option<WeilTriePrefixMap<T>> {
        let storage_prefix = self.state_tree_key(prefix);
        match suffix {
            Some(suffix) => {
                Memory::read_prefix_for_trie_with_suffix(storage_prefix, suffix.as_str())
            }
            None => Memory::read_prefix_for_trie(storage_prefix),
        }
    }

    /// Exact-key multi-get against the partitioned row. Each entry in
    /// `items` is the full user-facing column key (not a prefix); the
    /// returned [`WeilTriePrefixMap`] holds the matching (key, value)
    /// pairs returned by the host in a single SQL query.
    ///
    /// Intended for aggregation queries that need a known set of
    /// records from a row in one shot — collapses N FFI crossings
    /// into 1.
    pub fn get_items_with_suffix(
        &self,
        suffix: String,
        items: Vec<&str>,
    ) -> Option<WeilTriePrefixMap<T>> {
        let storage_items: Vec<String> = items
            .iter()
            .map(|p| self.state_tree_key(p))
            .collect();
        Memory::read_items_for_trie_with_suffix(storage_items, suffix.as_str())
    }

    /// Range variant of [`get_with_prefix_and_suffix`]. Returns every
    /// entry on the partition row `{contract_id}_{suffix}` whose
    /// column key falls in the closed byte-lex interval
    /// `[start_key, end_key]`. ONE host call, ONE SQL query,
    /// constant-size FFI payload regardless of window width.
    ///
    /// Use this — instead of [`get_items_with_suffix`] — when
    /// the keys are lex-sortable and span a contiguous band. The
    /// canonical case is a date window over `pad_me`-padded day-epoch
    /// keys: pass the start day's key and the end day's key, get every
    /// day in between with no per-day prefix enumeration.
    ///
    /// Lex ordering is on the **byte** representation of the key. For
    /// integer-coordinate keys (e.g. day epochs), zero-pad to a fixed
    /// width so lex sort matches numeric sort across digit-count
    /// boundaries. Reuse the `pad_me`-style helper already in your
    /// collection-key construction code.
    pub fn get_with_range_and_suffix(
        &self,
        suffix: String,
        start_key: &str,
        end_key: &str,
    ) -> Option<WeilTriePrefixMap<T>> {
        let storage_start = self.state_tree_key(start_key);
        let storage_end = self.state_tree_key(end_key);
        Memory::read_range_for_trie_with_suffix(storage_start, storage_end, suffix.as_str())
    }
}

impl<T> WeilType for WeilTrieMap<T> where T: Serialize + DeserializeOwned {}

impl<'a, T> WeilCollection<'a, str> for WeilTrieMap<T>
where
    T: Serialize + DeserializeOwned,
{
    type Key = String;

    fn base_state_path(&self) -> WeilId {
        self.state_id
    }

    fn state_tree_key(&'a self, suffix: &'a str) -> String {
        format!(
            "{}_{}",
            <WeilTrieMap<T> as WeilCollection<'_, str>>::base_state_path(self),
            suffix,
        )
    }
}
