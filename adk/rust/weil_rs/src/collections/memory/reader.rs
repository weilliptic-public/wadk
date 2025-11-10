use super::WeilMemory;
use std::io::{self, Read};

/// `WeilMemory` reader which implements `io::Read`.
pub struct WeilMemoryReader {
    memory: WeilMemory,
    position: usize,
}

impl WeilMemoryReader {
    /// Constructs a new `WeilMemoryReader`.
    pub fn new(memory: WeilMemory) -> Self {
        Self {
            memory,
            position: 0,
        }
    }

    /// Returns the underlying `WeilMemory` by taking the onwership of the reader.
    pub fn inner(self) -> WeilMemory {
        self.memory
    }
}

impl Read for WeilMemoryReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.memory.read(self.position as u32, buf);
        self.position += bytes_read;

        Ok(bytes_read)
    }
}
