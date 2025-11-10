use super::{map::WeilMap, WeilCollection, WeilId};
use crate::{runtime::Memory, traits::WeilType};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, hash::Hash};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WeilSet<T>(WeilMap<T, ()>);

impl<T> WeilSet<T> {
    pub fn new(id: WeilId) -> Self {
        WeilSet(WeilMap::new(id))
    }
}

impl<T> WeilSet<T>
where
    T: WeilType + Hash + Eq,
{
    pub fn insert(&mut self, value: T) {
        Memory::write_collection::<()>(self.state_tree_key(&value), ())
    }

    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        Memory::read_collection::<()>(self.state_tree_key(value)).is_some()
    }

    pub fn remove<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        Memory::delete_collection::<()>(self.state_tree_key(value)).is_some()
    }
}

impl<T: WeilType> WeilType for WeilSet<T> {}

impl<'a, T: WeilType, Q: 'a> WeilCollection<'a, Q> for WeilSet<T> {
    type Key = T;

    fn base_state_path(&self) -> WeilId {
        <WeilMap<T, ()> as WeilCollection<'_, Q>>::base_state_path(&self.0)
    }

    fn state_tree_key(&'a self, suffix: &'a Q) -> String
    where
        Q: Serialize,
        <Self as WeilCollection<'a, Q>>::Key: Borrow<Q>,
    {
        <WeilMap<T, ()> as WeilCollection<'_, Q>>::state_tree_key(&self.0, &suffix)
    }
}
