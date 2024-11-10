
use nightcrawler::Prim;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Perm(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PermBit {
    Unknown         = 0,
    Execute         = 1 << 0,
    Write           = 1 << 1,
    Read            = 1 << 2,
    ReadAfterWrite  = 1 << 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);


// Differential restoration:
// - Blocks refer to contiguous memory regions
// - The dirty bitmap tracks changes to any byte in a block
// - The entire block is restored if any byte is dirtied
/*
 Visual Explanation/Example:
 Memory Layout:
+-------------------+-------------------+-------------------+
|     Block 0       |     Block 1       |     Block 2       | ... 
+-------------------+-------------------+-------------------+
^                   ^                   ^
|                   |                   |
addr.0         addr.0+DIRTY_BLOCK_SIZE  addr.0+2*DIRTY_BLOCK_SIZE

Dirty Bitmap (Each bit represents a block):
+---------------------------------------------------------------+
| idx=0: 0010 0110 ...  0000 | idx=1: 0010 0000 ...  0000 | ...
+---------------------------------------------------------------+
         ^                 ^
         |                 |
        bit=63            bit=0

Scenario:
- Suppose `addr.0` points somewhere in the middle of Block 0 and the buffer spans into Block 1.
- `block_start` will be 0 and `block_end` will be 1.
- If these blocks are not marked dirty, they will be added to the list and their bits set in the bitmap.

Details:
- Calculating bitmap idx: `block / 64`
-- Each bitmap idx represents 64 blocks.
- Calculating bitmap bit: `block % 64`
-- Each bit represents a block. 

Notes:
- Instead of tracking individual bytes, we track blocks.
- This is because it is more space efficient than tracking individual bytes.
*/

// Repeated accesses to large contiguous memory regions perform better with larger block sizes.
// Smaller and sporadic accesses perform better with smaller block sizes.
// Size in bytes
const DIRTY_BLOCK_SIZE: usize = 0x80;

// Track changes in memory state
pub struct DirtyState {
    // Track the indexes of blocks that have been dirtied
    pub blocks: Vec<usize>,

    // Track which blocks have been dirtied
    // This makes it easier to search and check if a block has been dirtied
    // The alternative is to iterate through the dirty vector, 
    //   which is inefficient for large address spaces that may dirty many blocks
    //   or for frequent dirty checks such as when address is dirtied multiple times.
    bitmap: Vec<u64>,
}

impl DirtyState {
    // Create new dirty state
    pub fn new(size: usize) -> Self {
        Self {
            blocks: Vec::with_capacity(size / DIRTY_BLOCK_SIZE + 1),
            bitmap: vec![0u64; size / DIRTY_BLOCK_SIZE / 64 + 1],
        }
    }

    // Mark an address as dirty
    pub fn mark(&mut self, addr: VirtAddr, len: Option<usize>) -> Option<()>{
        let len: usize = len.unwrap_or(1);
        let block_start = addr.0 / DIRTY_BLOCK_SIZE;
        let block_end = addr.0.checked_add(len)? / DIRTY_BLOCK_SIZE;

        for block in block_start..=block_end {
            let idx = block / 64;
            let bit = block % 64;

            // Ignore if block is already dirty
            if self.bitmap[idx] & (1 << bit) != 0 {
                continue;
            }

            self.blocks.push(block);
            self.bitmap[idx] |= 1 << bit;
        }

        Some(())
    }
}

// An isolated memory management unit
pub struct Mmu {
    // Block of memory allocated for this address space
    // Offset 0 is address 0 in the guest address space
    memory: Vec<u8>,

    // Hold byte-level permissions
    permissions: Vec<Perm>,

    // Track dirty blocks for differential restoration
    // See explanation above.
    pub dirty: DirtyState,

    // Base address of the next allocation
    pub alloc_base: VirtAddr,

    // Active allocations
    allocations: HashMap<VirtAddr, usize>,
}

impl Mmu {
    // Create new address space with `size` bytes of memory
    pub fn new(size: usize) -> Self {
        Self {
            memory:      vec![0u8; size],
            permissions: vec![Perm(PermBit::Unknown as u8); size],
            dirty:       DirtyState::new(size),
            alloc_base:  VirtAddr(0x10000),
            allocations: HashMap::new(),
        }
    }

    // Fork from the existing MMU
    pub fn fork(&self) -> Self {
        let size = self.memory.len();

        Mmu {
            memory: self.memory.clone(),
            permissions: self.permissions.clone(),
            dirty: DirtyState::new(size),
            alloc_base: self.alloc_base,
            allocations: self.allocations.clone(),
        }
    }

    // Reset the address space to the state of another address space
    pub fn reset(&mut self, other: &Mmu) {
        for &block in &self.dirty.blocks {
            // Addr for block is block * DIRTY_BLOCK_SIZE
            let start = block * DIRTY_BLOCK_SIZE;
            let end = (block + 1) * DIRTY_BLOCK_SIZE;
            
            // Clear bitmap entry using idx
            // We ignore the bit because all blocks should be tracked
            //   and it's atleast a u64 write each time anyways
            let idx = block / 64;
            self.dirty.bitmap[idx] = 0;
            
            // Restore memory
            self.memory[start..end].copy_from_slice(&other.memory[start..end]);
            
            // Restore permissions
            self.permissions[start..end].copy_from_slice(&other.permissions[start..end]);
        }
        
        // Clear dirty blocks
        self.dirty.blocks.clear();
        
        // Restore allocation base
        self.alloc_base = other.alloc_base;
        
        // Clear active allocations
        self.allocations.clear();
        self.allocations.extend(other.allocations.iter());
    }

    // Get the size of an allocation
    pub fn get_alloc_size(&self, base: VirtAddr) -> Option<usize> {
        self.allocations.get(&base).copied()
    }
    
    // Allocate a region of memory as uninitialized permissions
    pub fn alloc(&mut self, size: usize) -> Option<VirtAddr> {
        // Size is 0x10 byte aligned (Add padding)
        let aligned_size = (size + 0xF) & !0xF;
        
        // Current allocation base
        let base = self.alloc_base;
        
        // No more memory
        if base.0 >= self.memory.len() {
            return None;
        }

        // Update allocation base
        self.alloc_base = VirtAddr(self.alloc_base.0.checked_add(aligned_size)?);

        // Check if ran out of memory
        if self.alloc_base.0 > self.memory.len() {
            return None;
        }

        // Mark the memory as un-initialized and writable
        // Notice the use of size instead of aligned_size
        // This is because compiler optimizations use the padding sometimes
        self.set_perms(base, size, Perm(PermBit::ReadAfterWrite as u8 | PermBit::Write as u8));

        // Track the allocation
        self.allocations.insert(base, size);

        println!("Allocated {:X} bytes at {:X}", size, base.0);
        println!("Allocations: {:X?}", self.allocations);

        Some(base)
    }

    // Set permissions for a region of memory
    pub fn set_perms(&mut self, addr: VirtAddr, size: usize, perm: Perm) -> Option<()> {
        self.permissions.get_mut(addr.0..addr.0.checked_add(size)?)?.iter_mut().for_each(|p| *p = perm);
        Some(())
    }

    // Write from `buf` to `addr`
    // Admin is a special permission that allows writing to non-living allocations
    pub fn write_from(&mut self, addr: VirtAddr, buf: &[u8]) -> Option<()> {
        // Check permissions for all bytes
        let perms = self.permissions.get_mut(addr.0..addr.0.checked_add(buf.len()).expect("Address overflow"))
            .expect("Failed to get permissions");

        // Check if all bytes are writable
        // Check if any bytes are ReadAfterWrite
        let mut is_raw = false;
        for (offset, &p) in perms.iter().enumerate() {
            if (p.0 & PermBit::Write as u8) == 0 {
                println!("Attempt to write to non-writable memory at offset 0x{:X} of addr 0x{:X} (@0x{:X}", offset, addr.0, addr.0 + offset);
                return None;
            }

            is_raw |= (p.0 & PermBit::ReadAfterWrite as u8) != 0;
        }

        // Perform write
        self.memory.get_mut(addr.0..addr.0.checked_add(buf.len()).expect("Address overflow"))
            .expect("Failed to get memory")
            .copy_from_slice(buf);

        // Mark dirty blocks
        self.dirty.mark(addr, Some(buf.len())).expect("Failed to mark dirty");

        // If any byte is ReadAfterWrite, mark the byte as Read
        if is_raw {
            perms.iter_mut().for_each(|p| {
                if (p.0 & PermBit::ReadAfterWrite as u8) != 0 {
                    p.0 |= (PermBit::Read as u8) & !(PermBit::ReadAfterWrite as u8);
                }
            });
        }
        

        Some(())
    }
    
    // Write sizeof T bytes from `val` to `addr`
    pub fn write<T: Prim>(&mut self, addr: VirtAddr, val: T) -> Option<()> {
        let tmp = unsafe { 
            core::slice::from_raw_parts(&val as *const T as *const u8, core::mem::size_of::<T>())
        };

        self.write_from(addr, tmp)
    }

    // Read from `addr` into `buf` with expected permissions
    pub fn read_with_perms(&self, addr: VirtAddr, buf: &mut [u8], expected_perm: Perm) -> Option<()> {
        // Check if any bytes aren't readable
        let perms = self.permissions.get(addr.0..addr.0.checked_add(buf.len())?)?;
        if expected_perm.0 != (PermBit::Unknown as u8) && perms.iter().any(|p| (p.0 & expected_perm.0) == 0) {
            return None;
        }

        buf.copy_from_slice(&self.memory.get(addr.0..addr.0.checked_add(buf.len())?)?);
        Some(())
    }

    // Read from `addr` into `buf`
    pub fn read_into(&self, addr: VirtAddr, buf: &mut [u8]) -> Option<()> {
        self.read_with_perms(addr, buf, Perm(PermBit::Read as u8))
    }

    // Read of sizeof T bytes from `addr` with expected permissions
    pub fn read_perms<T: Prim>(&self, addr: VirtAddr, expected_perms: Perm) -> Option<T> {
        let mut tmp = [0u8; 16]; // Largest supported primitive is u128
        self.read_with_perms(addr, &mut tmp[..std::mem::size_of::<T>()], expected_perms)?;
        Some(unsafe { core::ptr::read_unaligned(tmp.as_ptr() as *const T) })
    }

    // Read of sizeof T bytes from `addr`
    pub fn read<T: Prim>(&self, addr: VirtAddr) -> Option<T> {
        self.read_perms(addr, Perm(PermBit::Read as u8))
    }
}