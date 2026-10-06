use crate::traits::WeilType;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, fmt::Display};

pub mod map;
pub mod memory;
pub mod plottable;
pub mod set;
pub mod trie;
pub mod vec;

/// Unique Identifier for a particular Weil Collection.
///
/// <br>
///
/// # Note
/// Through out the WeilApplet execution lifetime (which would be potentially infinte as WeilApplet once deploys is immutable),
/// this identifier should never be same for two supposedly different Weil Collection, no matter if those collections if constructed in
/// different methods bounded to the contract state.
/// Failing to adhere to the above constraint is a logical flaw of the implementation and anything disastrous can happen including data
/// inside contract state being corrupted.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct WeilId(pub u32);

impl Display for WeilId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Generates unique `WeilId`.
/// <br>
///
/// # Usage
/// Typical usage of `WeilIdGenerator` is to keep it inside the applet state and call `next_id` anywhere
/// there is a need for unique `WeilId`.
/// All the structures provided by the `weil_rs` crate takes `WeilId` as arguments and so it's the responsiblity of the
/// programmer to provide it unique id, which can be easily achieved using `WeilIdGenerator` among other manually implemented
/// ways.
/// <br>
///
/// # Note
/// There should be one `WeilIdGenerator` per applet (inside applet state).
#[derive(Serialize, Deserialize)]
pub struct WeilIdGenerator {
    curr_id: u32,
}

impl WeilIdGenerator {
    pub fn new(base_id: WeilId) -> Self {
        Self { curr_id: base_id.0 }
    }

    /// Returns unique `WeilId`
    pub fn next_id(&mut self) -> WeilId {
        self.curr_id += 1;

        WeilId(self.curr_id)
    }
}

impl WeilType for WeilIdGenerator {}

/// Trait for all Weil Collections.
/// This basically contains the boilerplate methods for constructing platform side key from the corrosponding `WeilId`
/// attached with the collection.
trait WeilCollection<'a, Q: 'a + ?Sized>: WeilType {
    type Key;

    fn base_state_path(&self) -> WeilId;
    fn state_tree_key(&'a self, suffix: &'a Q) -> String
    where
        Q: Serialize,
        <Self as WeilCollection<'a, Q>>::Key: Borrow<Q>;
}
