use super::{WeilCollection, WeilId};
use crate::{errors::IndexOutOfBoundsError, runtime::Memory, traits::WeilType};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// A shallow light weight lazy data-structure mimicking `std::vec::Vec`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilVec<T> {
    state_id: WeilId,
    len: usize,
    phantom: PhantomData<T>,
}

impl<T> WeilVec<T> {
    /// Constructs a new `WeilVec<T>` with length 0.
    pub fn new(id: WeilId) -> Self {
        WeilVec {
            state_id: id,
            len: 0,
            phantom: PhantomData,
        }
    }

    /// Returns length of the `WeilVec<T>`.
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T: WeilType> WeilVec<T> {
    /// Appends an element to the back of a collection.
    pub fn push(&mut self, item: T) {
        Memory::write_collection(self.state_tree_key(&self.len), item);

        self.len += 1;
    }

    /// Returns the owned element at the index or None if out of bounds.
    /// The method is slightly different from `std::vec::Vec` `get`, where it returns
    /// the owned element rather than a reference. This makes the returned value a
    /// completely different owned object whose mutations won't reflect in the `WeilVec<T>`
    /// entry at the index.
    /// So if one wants to mutate the returned value and get it reflected in `WeilVec<T>`,
    /// call `set` at the index after the mutation of the returned owned value.
    ///
    /// <br>
    ///
    /// # Example
    ///
    /// ```
    /// let mut vec: WeilVec<Vec<usize>> = WeilVec::new(WeilId(0));
    ///
    /// vec.push(vec![1, 2, 3]);
    /// vec.push(vec![10, 3, 4]);
    ///
    /// let mut v = vec.get(0).unwrap(); // first entry which would be: [1, 2, 3]
    /// v.push(4); // this change won't reflect at the index 0 inside `vec`
    ///
    /// vec.set(0, v).unwrap(); // now it will set the updated value of `v` inside `vec`
    /// ```
    ///
    /// <br>
    ///
    /// # Note
    /// Even though the API returns an owned value, it does not mean that value will not be there
    /// inside the `WeilVec<T>` at the index after calling the method. This is due to how values of
    /// `Weil Collections` in general are persisted on the platform side.
    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        let val = Memory::read_collection(self.state_tree_key(&index)).unwrap();

        Some(val)
    }

    /// Sets the element at the index.
    ///
    /// # Error
    /// The method returns error if out of bounds.
    pub fn set(&self, index: usize, item: T) -> Result<(), IndexOutOfBoundsError> {
        if index >= self.len {
            return Err(IndexOutOfBoundsError {
                index,
                len: self.len,
            });
        }

        Memory::write_collection(self.state_tree_key(&index), item);

        Ok(())
    }

    /// Removes the last element and returns it, or None if it is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len() == 0 {
            return None;
        }

        let last_index = self.len() - 1;

        let val = Memory::delete_collection(self.state_tree_key(&last_index)).unwrap();
        self.len -= 1;

        Some(val)
    }

    /// Returns the iterator of the `WeilVec<T>`.
    pub fn iter(&self) -> WeilVecIter<T> {
        WeilVecIter {
            curr_index: 0,
            v_ref: self,
        }
    }
}

impl<T: WeilType> WeilType for WeilVec<T> {}

impl<'a, T: WeilType> WeilCollection<'a, usize> for WeilVec<T> {
    type Key = usize;

    fn base_state_path(&self) -> WeilId {
        self.state_id
    }

    fn state_tree_key(&'a self, suffix: &'a usize) -> String {
        format!(
            "{}_{}",
            <WeilVec<T> as WeilCollection<'_, usize>>::base_state_path(self),
            suffix
        )
    }
}

impl<T> Clone for WeilVec<T> {
    fn clone(&self) -> Self {
        WeilVec {
            state_id: self.state_id,
            len: self.len,
            phantom: PhantomData,
        }
    }
}

impl<'a, T: WeilType> IntoIterator for &'a WeilVec<T> {
    type Item = T;
    type IntoIter = WeilVecIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator for `WeilVec<T>`.
pub struct WeilVecIter<'a, T> {
    curr_index: usize,
    v_ref: &'a WeilVec<T>,
}

impl<'a, T: WeilType> Iterator for WeilVecIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.v_ref.get(self.curr_index)?;
        self.curr_index += 1;

        Some(result)
    }
}
