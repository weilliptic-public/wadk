use super::{WeilCollection, WeilId};
use crate::runtime::Memory;
use crate::traits::WeilType;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::{hash::Hash, marker::PhantomData};

/// A shallow light weight lazy data-structure mimicking `std::collections::HashMap`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilMap<K, V> {
    state_id: WeilId,
    phantom: PhantomData<(K, V)>,
}

impl<K, V> WeilMap<K, V> {
    /// Constructs a new empty `WeilMap<K, V>`.
    pub fn new(id: WeilId) -> Self {
        Self {
            state_id: id,
            phantom: PhantomData,
        }
    }
}

impl<K, V> WeilMap<K, V>
where
    K: Serialize + DeserializeOwned + Hash + Eq,
    V: WeilType,
{
    /// Inserts a key-value pair into the map.
    /// If the map did have this key present, the value is updated.
    pub fn insert(&mut self, key: K, value: V) -> Result<(), String> {
        self.do_insert(None, key, value)
    }

    pub fn insert_with_suffix(&mut self, suffix: String, key: K, value: V) -> Result<(), String> {
        self.do_insert(Some(suffix), key, value)
    }

    fn do_insert(&mut self, suffix: Option<String>, key: K, value: V) -> Result<(), String> {
        let storage_key = self.state_tree_key(&key);
        match suffix {
            Some(suffix) => {
                Memory::write_collection_with_suffix(storage_key, value, suffix.as_str())
            }
            None => Memory::write_collection(storage_key, value),
        }
    }

    /// Returns the owned value corresponding to the key.
    /// The method is slightly different from `std::collections::HashMap` `get`, where it returns
    /// the owned element rather than a reference. This makes the returned value a
    /// completely different owned object whose mutations won't reflect in the `WeilMap<K, V>`
    /// value corrosponding to the key.
    /// So if one wants to mutate the returned value and get it reflected in `WeilMap<K, V>`,
    /// call `insert` after the mutation of the returned owned value.
    ///
    /// <br>
    ///
    /// # Example
    ///
    /// ```
    /// let mut map: WeilMap<String, Vec<usize>> = WeilMap::new(WeilId(0));
    ///
    /// map.insert("key1".to_string(), vec![1, 2, 3]);
    /// map.insert("key2".to_string(), vec![10, 3, 4]);
    ///
    /// let mut v = map.get(&"key1".to_string()).unwrap(); // this would be: [1, 2, 3]
    /// v.push(4); // this change won't reflect in the value for the key `key1`
    ///
    /// map.insert("key1".to_string(), v); // now it will set the updated value of `v` inside `map`
    /// ```
    ///
    /// <br>
    ///
    /// # Note
    /// Even though the API returns an owned value, it does not mean that value will not be there
    /// inside the `WeilVec<T>` at the index after calling the method. This is due to how values of
    /// `Weil Collections` in general are persisted on the platform side.
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        self.do_get(key, None)
    }

    pub fn get_with_suffix<Q>(&self, suffix: String, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        self.do_get(key, Some(suffix))
    }

    fn do_get<Q>(&self, key: &Q, suffix: Option<String>) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        let storage_key = self.state_tree_key(key);
        match suffix {
            Some(suffix) => Memory::read_collection_with_suffix(storage_key, suffix.as_str()),
            None => Memory::read_collection(storage_key),
        }
    }

    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    pub fn remove<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        self.do_remove(None, key)
    }

    pub fn remove_with_suffix<Q>(&self, suffix: String, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        self.do_remove(Some(suffix), key)
    }

    fn do_remove<Q>(&self, suffix: Option<String>, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        let storage_key = self.state_tree_key(key);
        match suffix {
            Some(suffix) => Memory::delete_collection_with_suffix(storage_key, suffix.as_str()),
            None => Memory::delete_collection(storage_key),
        }
    }

    /// Batch exact-key multi-get. ONE host call returns a `HashMap`
    /// of every requested key present on the target row.
    ///
    /// `suffix`:
    ///   - `Some(s)` — partitioned row `{contract_id}_{s}`
    ///   - `None`    — default contract row
    ///
    /// Mirrors the `Q: Borrow<K>` pattern of [`Self::get`] — callers
    /// with `K = String` can pass `&[&str]`; callers with enum/struct
    /// `K` pass `&[K]` directly. Each user key's `state_tree_key`
    /// becomes the exact column name queried; missing keys are absent
    /// from the returned `HashMap`.
    ///
    /// Intended for known-set aggregation reads — e.g.
    /// `keys_by_purpose.getN(None, &[Execution, Management])` on
    /// `KeyManager` (one host call returns both purpose buckets), or
    /// `records.getN(None, &owned_namehashes)` on Registry for
    /// `get_owner_summary`.
    pub fn getN<Q>(
        &self,
        suffix: Option<String>,
        keys: &[Q],
    ) -> std::collections::HashMap<K, V>
    where
        K: Borrow<Q> + Clone,
        Q: Hash + Eq + Serialize,
        V: DeserializeOwned + Clone,
    {
        // Platform-side column names go out untrimmed (full state_tree_key)
        // for the DB lookup but come back TRIMMED — `trim_state_id` strips
        // the `{state_id}_` prefix before the host serializes the response.
        // So `raw` is keyed by the trimmed form (= `serde_json::to_string(q)`),
        // not by the full state_tree_key. See
        // `w_scruntime/src/collection_manager.rs::trim_state_id` and the
        // `read_items_*_from_db` helpers.
        let storage_keys: Vec<String> =
            keys.iter().map(|q| self.state_tree_key(q)).collect();

        let raw: Option<std::collections::HashMap<String, V>> = match suffix {
            Some(s) => Memory::read_items_for_map_with_suffix(&storage_keys, s.as_str()),
            None => Memory::read_items_for_map(&storage_keys),
        };

        let Some(raw) = raw else { return std::collections::HashMap::new(); };

        // Reproject (trimmed column name → V) → (K → V).
        // The lookup key in `raw` is the trimmed form, not the full
        // state_tree_key. Cost per requested key: one serialize for the
        // lookup, one serde round-trip to mint owned K. Both happen on
        // cached-deserialized values — no host crossings.
        keys.iter()
            .filter_map(|q| {
                let trimmed_key = serde_json::to_string(q).ok()?;
                let v = raw.get(&trimmed_key)?.clone();
                let owned_k: K =
                    serde_json::from_value(serde_json::to_value(q).ok()?).ok()?;
                Some((owned_k, v))
            })
            .collect()
    }
}

impl<K, V> WeilType for WeilMap<K, V>
where
    K: Serialize + DeserializeOwned,
    V: WeilType,
{
}

impl<'a, K, V, Q: 'a> WeilCollection<'a, Q> for WeilMap<K, V>
where
    K: Serialize + DeserializeOwned,
    V: WeilType,
{
    type Key = K;

    fn base_state_path(&self) -> WeilId {
        self.state_id
    }

    fn state_tree_key(&'a self, suffix: &'a Q) -> String
    where
        Q: Serialize,
        <Self as WeilCollection<'a, Q>>::Key: Borrow<Q>,
    {
        format!(
            "{}_{}",
            <WeilMap<K, V> as WeilCollection<'_, Q>>::base_state_path(self),
            serde_json::to_string(suffix).unwrap()
        )
    }
}

impl<K, V> Clone for WeilMap<K, V> {
    fn clone(&self) -> Self {
        WeilMap {
            state_id: self.state_id,
            phantom: PhantomData,
        }
    }
}
