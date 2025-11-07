use crate::traits::WeilType;
use bytesize::KIB;
use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

pub const DEFAULT_CHUNK_SIZE: u32 = 4 * KIB as u32; // default max length of a Chunk

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Chunk(Vec<u8>);

impl Chunk {
    pub fn new(chunk: Vec<u8>) -> Self {
        Chunk(chunk)
    }

    pub fn chunk_size(&self) -> u32 {
        self.0.len() as u32
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl Index<usize> for Chunk {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Chunk {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl WeilType for Chunk {}
