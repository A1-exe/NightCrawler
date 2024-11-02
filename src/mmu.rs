// Permission bits

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Perm(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PermBit {
    Unknown         = 0,
    Execute         = 1 << 0,
    Write           = 1 << 1,
    Read            = 1 << 2,
    ReadAfterWrite  = 1 << 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);

// An isolated memory management unit
pub struct Mmu {
    // Block of memory allocated for this address space
    // Offset 0 is address 0 in the guest address space
    memory: Vec<u8>,

    // Hold byte-level permissions
    permissions: Vec<Perm>,

    // Base address of the next allocation
    alloc_base: VirtAddr,
}

impl Mmu {
    // Create new address space with `size` bytes of memory
    pub fn new(size: usize) -> Self {
        Self {
            memory:      vec![0; size],
            permissions: vec![Perm(PermBit::Unknown as u8); size],
            alloc_base:  VirtAddr(0x10000),
        }
    }

    pub fn get_base(&self) -> VirtAddr {
        self.alloc_base
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

        Some(base)
    }

    // Set permissions for a region of memory
    pub fn set_perms(&mut self, addr: VirtAddr, size:usize, perm: Perm) -> Option<()> {
        self.permissions.get_mut(addr.0..addr.0.checked_add(size)?)?.iter_mut().for_each(|p| *p = perm);
        Some(())
    }

    // Write from `buf` to `addr`
    pub fn write(&mut self, addr: VirtAddr, buf: &[u8]) -> Option<()> {
        let mut perms = self.permissions.get_mut(addr.0..addr.0.checked_add(buf.len())?)?;

        // Check if all bytes are writable
        // Check if any bytes are ReadAfterWrite
        let mut is_raw = false;
        if !perms.iter().all(|p| {
            is_raw |= (p.0 & PermBit::ReadAfterWrite as u8) != 0;
            (p.0 & PermBit::Write as u8) != 0
        }) {
            return None;
        }

        self.memory.get_mut(addr.0..addr.0.checked_add(buf.len())?)?
            .copy_from_slice(buf);

        // If any byte is ReadAfterWrite, mark the byte as Read
        if is_raw {
            perms.iter_mut().for_each(|p| {
                if (p.0 & PermBit::ReadAfterWrite as u8) != 0 {
                    p.0 |= PermBit::Read as u8;
                }
            });
        }
        

        Some(())
    }

    // Read from `addr` to `buf`
    pub fn read(&self, addr: VirtAddr, buf: &mut [u8]) -> Option<()> {
        // Check if any bytes aren't readable
        let perms = self.permissions.get(addr.0..addr.0.checked_add(buf.len())?)?;
        if perms.iter().any(|p| (p.0 & PermBit::Read as u8) == 0) {
            return None;
        }

        buf.copy_from_slice(&self.memory.get(addr.0..addr.0.checked_add(buf.len())?)?);
        Some(())
    }

}