pub mod emu;
pub mod mmu;

use emu::Emulator;


fn main() {
    let mut emu = Emulator::new(1024 * 1024);
    
    let tmp = emu.memory.alloc(6).unwrap();
    emu.memory.write(mmu::VirtAddr(tmp.0), b"meeper").expect("Failed to write");
    let base_emu = emu.fork();
    
    let mut bts = [0u8; 6];
    emu.memory.read(tmp, &mut bts).expect("Failed to read");
    println!("Dirtied {:?}", bts);

    emu.reset(&base_emu);

    let mut bts = [0u8; 6];
    emu.memory.read(tmp, &mut bts).unwrap();
    println!("After {:?}", bts);

    println!("{:x?}", tmp);
    println!("Dirty: {:?}", emu.memory.dirty.blocks);
}
