use crate::mmu::Mmu;

// Emulated process/system state
pub struct Emulator {
    pub memory: Mmu,
}

impl Emulator {
    // Create new emulator with `size` bytes of memory
    pub fn new(size: usize) -> Self {
        Self {
            memory: Mmu::new(size),
        }
    }
}