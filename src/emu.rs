use elf::ElfBytes;

use crate::mmu::{Mmu, VirtAddr, Perm, PermBit};

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

    // Load a program into memory
    pub fn load(&mut self, filename: &str) -> Option<VirtAddr> {
        // Parse ELF file for LOAD sections
        let file_contents = std::fs::read(filename).ok()?;

        use elf::endian::AnyEndian;
        use elf::ElfBytes;
        let file = ElfBytes::<AnyEndian>::minimal_parse(&file_contents.as_slice()).unwrap();

        for header in file.segments().expect("Failed to read segments") {
            // Only load LOAD segments
            if header.p_type != 1 {
                continue;
            }

            let file_offset = header.p_offset as usize;
            let virt_addr = VirtAddr(header.p_vaddr as usize);
            let file_size = header.p_filesz as usize;
            let mem_size = header.p_memsz as usize;
            let perms = header.p_flags as u8;

            println!("Loading segment:");
            println!("{:X?}", header);

            // Store the segment in emulated memory
            // ** No need to mark as a live allocation since it'll restored when memory is cloned
            // ** Bounds checking on these allocations is unnecessary since off-by-one in these areas wouldn't be exploitable
            // ** Arbitrarily controlled read or writes relative to these areas would be detected by perms checks anyways
            self.memory.set_perms(virt_addr, mem_size, Perm(PermBit::Write as u8))?;

            // Write from file to memory
            self.memory.write(virt_addr, &file_contents[file_offset..file_offset.checked_add(file_size)?], Some(()))?;

            // Write 0 padding if necessary
            if file_size < mem_size {
                self.memory.write(VirtAddr(virt_addr.0.checked_add(file_size)?), &vec![0; mem_size - file_size], Some(()))?;
            }

            // Set appropriate permissions
            self.memory.set_perms(virt_addr, mem_size, Perm(perms))?;

            // Move allocator beyond loaded sections
            self.memory.alloc_base = VirtAddr(std::cmp::max(
                self.memory.alloc_base.0, 
                virt_addr.0.checked_add(mem_size + 0xFFF)? & !0xFFF)
            );
        }

        // Return entry point
        Some(VirtAddr(file.ehdr.e_entry as usize))
    }
}