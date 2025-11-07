use super::WeilMemory;
use std::io::{self, Write};

/// `WeilMemory` writer which implements `io::Write`.
pub struct WeilMemoryWriter {
    memory: WeilMemory,
    position: usize,
}

impl WeilMemoryWriter {
    /// Constructs a new `WeilMemoryWriter`.
    pub fn new(memory: WeilMemory) -> Self {
        Self {
            memory,
            position: 0,
        }
    }

    /// Returns the underlying `WeilMemory` by taking the onwership of the writer.
    pub fn inner(self) -> WeilMemory {
        self.memory
    }
}

impl Write for WeilMemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_write = self.memory.write(self.position as u32, buf);
        self.position += bytes_write;

        Ok(bytes_write)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
