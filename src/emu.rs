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

    // Fork from the existing emulator
    pub fn fork(&self) -> Self {
        Self {
            memory: self.memory.fork(),
        }
    }

    // Reset the emulator to the state of another emulator
    pub fn reset(&mut self, other: &Self) {
        self.memory = other.memory.fork();
    }
}