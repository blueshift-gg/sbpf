use {
    crate::errors::{VmError, VmResult},
    serde::{Deserialize, Serialize},
};

/// Memory region
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegion {
    Input,
    Rodata,
    Stack,
    Heap,
}

/// Memory layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub rodata: Vec<u8>,
    pub stack: Vec<u8>,
    pub heap: Vec<u8>,
    pub input: Vec<u8>,
    pub heap_ptr: usize,
}

impl Memory {
    // Virtual address memory map
    pub const RODATA_START: u64 = 0x100000000; // Read-only data (rodata)
    pub const STACK_START: u64 = 0x200000000; // Stack data
    pub const HEAP_START: u64 = 0x300000000; // Heap data
    pub const INPUT_START: u64 = 0x400000000; // Program input parameters

    pub const DEFAULT_STACK_SIZE: usize = 4096; // 4KB
    pub const DEFAULT_HEAP_SIZE: usize = 32768; // 32KB
    pub const STACK_FRAME_SIZE: u64 = 4096; // 4KB

    pub fn new(input: Vec<u8>, rodata: Vec<u8>, stack_size: usize, heap_size: usize) -> Self {
        Self {
            input,
            rodata,
            stack: vec![0u8; stack_size],
            heap: vec![0u8; heap_size],
            heap_ptr: 0,
        }
    }

    pub fn initial_frame_pointer(&self) -> u64 {
        Self::STACK_START + self.stack.len() as u64
    }

    // Translate virtual address to region and offset
    fn translate(&self, addr: u64) -> VmResult<(MemoryRegion, usize)> {
        if addr >= Self::INPUT_START {
            let offset = (addr - Self::INPUT_START) as usize;
            if offset < self.input.len() {
                Ok((MemoryRegion::Input, offset))
            } else {
                Err(VmError::MemoryOutOfBounds(addr, 0))
            }
        } else if addr >= Self::HEAP_START {
            let offset = (addr - Self::HEAP_START) as usize;
            if offset < self.heap.len() {
                Ok((MemoryRegion::Heap, offset))
            } else {
                Err(VmError::MemoryOutOfBounds(addr, 0))
            }
        } else if addr >= Self::STACK_START {
            let offset = (addr - Self::STACK_START) as usize;
            if offset < self.stack.len() {
                Ok((MemoryRegion::Stack, offset))
            } else {
                Err(VmError::MemoryOutOfBounds(addr, 0))
            }
        } else if addr >= Self::RODATA_START {
            let offset = (addr - Self::RODATA_START) as usize;
            if offset < self.rodata.len() {
                Ok((MemoryRegion::Rodata, offset))
            } else {
                Err(VmError::MemoryOutOfBounds(addr, 0))
            }
        } else {
            Err(VmError::InvalidMemoryAccess(addr))
        }
    }

    fn get_slice(&self, region: MemoryRegion, offset: usize, len: usize) -> VmResult<&[u8]> {
        let data = match region {
            MemoryRegion::Input => &self.input,
            MemoryRegion::Rodata => &self.rodata,
            MemoryRegion::Stack => &self.stack,
            MemoryRegion::Heap => &self.heap,
        };

        if offset + len > data.len() {
            return Err(VmError::MemoryOutOfBounds(offset as u64, len));
        }

        Ok(&data[offset..offset + len])
    }

    fn get_slice_mut(
        &mut self,
        region: MemoryRegion,
        offset: usize,
        len: usize,
    ) -> VmResult<&mut [u8]> {
        // Rodata region is read-only
        if region == MemoryRegion::Rodata {
            return Err(VmError::InvalidMemoryAccess(
                Self::RODATA_START + offset as u64,
            ));
        }

        let data = match region {
            MemoryRegion::Input => &mut self.input,
            MemoryRegion::Stack => &mut self.stack,
            MemoryRegion::Heap => &mut self.heap,
            MemoryRegion::Rodata => unreachable!(),
        };

        if offset + len > data.len() {
            return Err(VmError::MemoryOutOfBounds(offset as u64, len));
        }

        Ok(&mut data[offset..offset + len])
    }

    pub fn read_u8(&self, addr: u64) -> VmResult<u8> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice(region, offset, 1)?;
        Ok(slice[0])
    }

    pub fn read_u16(&self, addr: u64) -> VmResult<u16> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice(region, offset, 2)?;
        Ok(u16::from_le_bytes([slice[0], slice[1]]))
    }

    pub fn read_u32(&self, addr: u64) -> VmResult<u32> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice(region, offset, 4)?;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    pub fn read_u64(&self, addr: u64) -> VmResult<u64> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice(region, offset, 8)?;
        Ok(u64::from_le_bytes([
            slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
        ]))
    }

    pub fn read_bytes(&self, addr: u64, len: usize) -> VmResult<&[u8]> {
        let (region, offset) = self.translate(addr)?;
        self.get_slice(region, offset, len)
    }

    pub fn write_u8(&mut self, addr: u64, value: u8) -> VmResult<()> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice_mut(region, offset, 1)?;
        slice[0] = value;
        Ok(())
    }

    pub fn write_u16(&mut self, addr: u64, value: u16) -> VmResult<()> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice_mut(region, offset, 2)?;
        slice.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u32(&mut self, addr: u64, value: u32) -> VmResult<()> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice_mut(region, offset, 4)?;
        slice.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u64(&mut self, addr: u64, value: u64) -> VmResult<()> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice_mut(region, offset, 8)?;
        slice.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_bytes(&mut self, addr: u64, bytes: &[u8]) -> VmResult<()> {
        let (region, offset) = self.translate(addr)?;
        let slice = self.get_slice_mut(region, offset, bytes.len())?;
        slice.copy_from_slice(bytes);
        Ok(())
    }

    pub fn alloc(&mut self, size: usize) -> VmResult<u64> {
        if self.heap_ptr + size > self.heap.len() {
            return Err(VmError::MemoryOutOfBounds(
                Self::HEAP_START + self.heap_ptr as u64,
                size,
            ));
        }
        let addr = Self::HEAP_START + self.heap_ptr as u64;
        self.heap_ptr += size;
        Ok(addr)
    }

    pub fn reset_heap(&mut self) {
        self.heap_ptr = 0;
        self.heap.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_regions() {
        let input = vec![1, 2, 3, 4];
        let rodata = vec![5, 6, 7, 8];
        let memory = Memory::new(input, rodata, 1024, 1024);

        // Test input and rodata region
        assert_eq!(memory.read_u8(Memory::INPUT_START).unwrap(), 1);
        assert_eq!(memory.read_u8(Memory::INPUT_START + 3).unwrap(), 4);

        assert_eq!(memory.read_u8(Memory::RODATA_START).unwrap(), 5);
        assert_eq!(memory.read_u8(Memory::RODATA_START + 3).unwrap(), 8);
    }

    #[test]
    fn test_read_write() {
        let mut memory = Memory::new(vec![0; 16], vec![0; 16], 1024, 1024);

        let fp = memory.initial_frame_pointer();

        // Write and read u8
        let addr = fp - 1;
        memory.write_u8(addr, 0x5).unwrap();
        assert_eq!(memory.read_u8(addr).unwrap(), 0x5);

        // Write and read u16
        let addr = fp - 2;
        memory.write_u16(addr, 0xabcd).unwrap();
        assert_eq!(memory.read_u16(addr).unwrap(), 0xabcd);

        // Write and read u32
        let addr = fp - 4;
        memory.write_u32(addr, 0xabcd1234).unwrap();
        assert_eq!(memory.read_u32(addr).unwrap(), 0xabcd1234);

        // Write and read u64
        let addr = fp - 8;
        memory.write_u64(addr, 0x123456789abcdef0).unwrap();
        assert_eq!(memory.read_u64(addr).unwrap(), 0x123456789abcdef0);
    }

    #[test]
    fn test_heap_allocation() {
        let mut memory = Memory::new(vec![], vec![], 1024, 1024);

        let addr1 = memory.alloc(64).unwrap();
        assert_eq!(addr1, Memory::HEAP_START);

        let addr2 = memory.alloc(128).unwrap();
        assert_eq!(addr2, Memory::HEAP_START + 64);

        memory.write_u64(addr1, 0x12345678).unwrap();
        assert_eq!(memory.read_u64(addr1).unwrap(), 0x12345678);
    }

    #[test]
    fn test_rodata_readonly() {
        let mut memory = Memory::new(vec![], vec![1, 2, 3, 4], 1024, 1024);

        // should fail to write to read-only region
        let result = memory.write_u8(Memory::RODATA_START, 12);
        assert!(result.is_err());
    }
}
