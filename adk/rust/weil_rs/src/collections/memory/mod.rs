use super::{vec::WeilVec, WeilCollection, WeilId};
use crate::{
    errors::{ChunkSizeError, InvalidChunkSizeError},
    traits::WeilType,
};
use bytesize::KIB;
use chunk::Chunk;
use serde::{Deserialize, Serialize};

pub mod chunk;
pub mod reader;
pub mod writer;

/// Index for the `WeilMemory` when viewed as `WeilVec<Chunk>`.
pub struct ChunkIndex(pub u32);

/// A collection internally described as `WeilVec<Chunk>` which enable wider resolution for each element (typically called chunks with size 4KiB).
/// `WeilMemory` with its properties can model a storage layer to store large amount of data like files, databases etc.
/// `WeilMemory` can be viewed in two different way:
/// - `Vec<u8>`: provided by methods like `read` and `write`.
/// - `WeilVec<Chunk>`: provided by methods like `chunk` and `set_chunk`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilMemory {
    memory: WeilVec<Chunk>,
    chunk_size: u32,
}

impl WeilMemory {
    /// * `id` - The unique identifier for this memory collection
    /// * `num_chunks` - The number of chunks to initialize with
    /// * `chunk_size` - chunk size in bytes
    ///
    /// <br>
    ///
    /// Constructs a new `WeilMemory` with total chunks when viewed as `WeilVec<Chunk>`.
    ///
    /// # Returns
    /// A new `WeilMemory` instance initialized with `num_chunks` zero-filled chunks
    ///
    /// # Error
    /// This returns error if chunk size is beyond 64 KiB (1 WASM page).
    ///
    /// # Note
    /// For most programs chunk size of 4 KiB or 8 KiB or 16 KiB is good enough to attain optimal tradeoff between space and time.
    /// A typical example would be to save a file on-chain inside the `WeilMemory` and so while uploading the file, one would chunk it
    /// and send potentially concurrent client requests to the contract which might use `set_chunk` (see below) method. So using a chunk size
    /// of 16 KiB can be optimal for fast upload speed as well as keeping well within wasm memory bounds.
    pub fn with_num_chunks(
        id: WeilId,
        num_chunks: u32,
        chunk_size: u32,
    ) -> Result<Self, ChunkSizeError> {
        if chunk_size as u64 > 64 * KIB {
            return Err(ChunkSizeError {
                received_size: chunk_size,
            });
        }

        let mut mem = WeilMemory {
            memory: WeilVec::new(id),
            chunk_size,
        };

        let chunk_size = mem.chunk_size;

        for _ in 0..num_chunks {
            mem.memory.push(Chunk::new(vec![0; chunk_size as usize]));
        }

        Ok(mem)
    }

    /// Returns the chunk at the index when `WeilMemory` is viewed as `WeilVec<Chunk>`.
    pub fn chunk(&self, index: ChunkIndex) -> Option<Vec<u8>> {
        let Some(chunk) = self.memory.get(index.0 as usize) else {
            return None;
        };

        Some(chunk.into_vec())
    }

    /// Sets the chunk at the index when `WeilMemory` is viewed as `WeilVec<Chunk>`.
    ///
    /// <Br>
    ///
    /// # Error
    /// This function returns error if:
    /// - the index provided is out of bounds.
    /// - the size of the chunk does not matches with the expected chunk size.
    pub fn set_chunk(&self, index: ChunkIndex, chunk: Vec<u8>) -> Result<(), anyhow::Error> {
        if chunk.len() != self.chunk_size as usize {
            return Err(InvalidChunkSizeError {
                expected_size: self.chunk_size,
                received_size: chunk.len() as u32,
            }
            .into());
        }

        self.memory.set(index.0 as usize, Chunk::new(chunk))?;

        Ok(())
    }

    /// Appends a chunk to the back of a `WeilMemory` when viewed as `WeilVec<Chunk>`.
    ///
    /// <Br>
    ///
    /// # Error
    /// This function returns error if the size of the chunk does not matches with the expected chunk size.
    pub fn push(&mut self, chunk: Vec<u8>) -> Result<(), InvalidChunkSizeError> {
        if chunk.len() != self.chunk_size as usize {
            return Err(InvalidChunkSizeError {
                expected_size: self.chunk_size,
                received_size: chunk.len() as u32,
            }
            .into());
        }

        self.memory.push(Chunk::new(chunk));

        Ok(())
    }

    fn chunk_index(&self, offset: u32) -> ChunkIndex {
        let chunk_index = offset / self.chunk_size;

        ChunkIndex(chunk_index)
    }

    fn compute_chunk_span(&self, offset: u32, critical_len: usize) -> ChunkSpanResult {
        let chunk_index = self.chunk_index(offset);
        let start_index = chunk_index.0 * self.chunk_size;
        let relative_offset = offset - start_index;

        if relative_offset + critical_len as u32 <= self.chunk_size {
            return ChunkSpanResult::SameChunk {
                start_chunk_index: chunk_index.0,
                start_index: relative_offset,
            };
        } else {
            let consumed_len = self.chunk_size - relative_offset;
            let remaining_len = critical_len as u32 - consumed_len;
            let num_in_between_chunks = remaining_len / self.chunk_size;
            let end_index = remaining_len - (num_in_between_chunks * self.chunk_size);

            return ChunkSpanResult::MultipleChunks {
                start_chunk_index: chunk_index.0,
                start_index: relative_offset,
                in_between_chunks: num_in_between_chunks,
                end_index,
            };
        }
    }

    /// Reads the data from the offset to the destination buffer (`WeilMemory` viewed as `Vec<u8>`).
    /// This signature follows the `io::Read` convention. See `WeilMemoryReader` for more info.
    pub fn read(&self, offset: u32, dst: &mut [u8]) -> usize {
        let mem_len = self.total_chunks() * self.chunk_size;

        if offset >= mem_len {
            return 0;
        }

        let available = mem_len - offset;

        let bytes_to_read = if available as usize >= dst.len() {
            dst.len()
        } else {
            available as usize
        };

        let chunk_span_result = self.compute_chunk_span(offset, bytes_to_read);

        match chunk_span_result {
            ChunkSpanResult::SameChunk {
                start_chunk_index,
                start_index,
            } => {
                let chunk = self.chunk(ChunkIndex(start_chunk_index)).unwrap();

                for i in 0..bytes_to_read {
                    dst[i] = chunk[start_index as usize + i];
                }
            }
            ChunkSpanResult::MultipleChunks {
                start_chunk_index,
                start_index,
                in_between_chunks,
                end_index,
            } => {
                let chunk = self.chunk(ChunkIndex(start_chunk_index)).unwrap();
                let mut curr_index = 0;

                for i in start_index..self.chunk_size {
                    dst[curr_index] = chunk[i as usize];
                    curr_index += 1;
                }

                let mut curr_chunk_index = start_chunk_index + 1;

                for _ in 0..in_between_chunks {
                    let chunk = self.chunk(ChunkIndex(curr_chunk_index)).unwrap();

                    for i in 0..self.chunk_size {
                        dst[curr_index] = chunk[i as usize];
                        curr_index += 1;
                    }

                    curr_chunk_index += 1;
                }

                let chunk = self.chunk(ChunkIndex(curr_chunk_index)).unwrap();

                for i in 0..end_index {
                    dst[curr_index] = chunk[i as usize];
                    curr_index += 1;
                }

                debug_assert!(curr_index == bytes_to_read);
            }
        }

        bytes_to_read
    }

    /// Writes the data at the offset from the source buffer (`WeilMemory` viewed as `Vec<u8>`).
    /// This signature follows the `io::Write` convention. See `WeilMemoryWriter` for more info.
    pub fn write(&self, offset: u32, src: &[u8]) -> usize {
        let mem_len = self.total_chunks() * self.chunk_size;

        if offset >= mem_len {
            return 0;
        }

        let available = mem_len - offset;

        let bytes_to_write = if available as usize >= src.len() {
            src.len()
        } else {
            available as usize
        };

        let chunk_span_result = self.compute_chunk_span(offset, bytes_to_write);

        match chunk_span_result {
            ChunkSpanResult::SameChunk {
                start_chunk_index,
                start_index,
            } => {
                let mut chunk = self.chunk(ChunkIndex(start_chunk_index)).unwrap();

                for i in 0..bytes_to_write {
                    chunk[start_index as usize + i] = src[i];
                }

                self.set_chunk(ChunkIndex(start_chunk_index), chunk)
                    .unwrap();
            }
            ChunkSpanResult::MultipleChunks {
                start_chunk_index,
                start_index,
                in_between_chunks,
                end_index,
            } => {
                let mut chunk = self.chunk(ChunkIndex(start_chunk_index)).unwrap();
                let mut curr_index = 0;

                for i in start_index..self.chunk_size {
                    chunk[i as usize] = src[curr_index];
                    curr_index += 1;
                }

                self.set_chunk(ChunkIndex(start_chunk_index), chunk)
                    .unwrap();

                let mut curr_chunk_index = start_chunk_index + 1;

                for _ in 0..in_between_chunks {
                    let mut chunk = self.chunk(ChunkIndex(curr_chunk_index)).unwrap();

                    for i in 0..self.chunk_size {
                        chunk[i as usize] = src[curr_index];
                        curr_index += 1;
                    }

                    self.set_chunk(ChunkIndex(curr_chunk_index), chunk).unwrap();

                    curr_chunk_index += 1;
                }

                let mut chunk = self.chunk(ChunkIndex(curr_chunk_index)).unwrap();

                for i in 0..end_index {
                    chunk[i as usize] = src[curr_index];
                    curr_index += 1;
                }

                self.set_chunk(ChunkIndex(curr_chunk_index), chunk).unwrap();

                debug_assert!(curr_index == bytes_to_write);
            }
        }

        bytes_to_write
    }

    /// Returns the total chunks of the `WeilMemory`.
    /// The following relation always hold `len <= CHUNK_SIZE * total_chunks`.
    pub fn total_chunks(&self) -> u32 {
        self.memory.len() as u32
    }
}

impl WeilType for WeilMemory {}

impl<'a> WeilCollection<'a, usize> for WeilMemory {
    type Key = usize;

    fn base_state_path(&self) -> WeilId {
        <WeilVec<Chunk> as WeilCollection<'_, usize>>::base_state_path(&self.memory)
    }

    fn state_tree_key(&'a self, suffix: &'a usize) -> String {
        <WeilVec<Chunk> as WeilCollection<'_, usize>>::state_tree_key(&self.memory, &suffix)
    }
}

impl Clone for WeilMemory {
    fn clone(&self) -> Self {
        WeilMemory {
            memory: self.memory.clone(),
            chunk_size: self.chunk_size,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ChunkSpanResult {
    SameChunk {
        start_chunk_index: u32,
        start_index: u32,
    },
    MultipleChunks {
        start_chunk_index: u32,
        start_index: u32,
        in_between_chunks: u32,
        end_index: u32,
    },
}
