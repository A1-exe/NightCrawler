pub mod emu;
pub mod mmu;

use emu::Emulator;
use nightcrawler::Rng;


fn main() {
    let mut rng = Rng::new();

    let mut emu = Emulator::new(1024 * 1024);

    let tmp = emu.memory.alloc(0x1000).unwrap();
    emu.memory.write(mmu::VirtAddr(tmp.0), b"meeper");

    let mut bts = [0u8; 6];
    emu.memory.read(tmp, &mut bts).unwrap();

    println!("{:x?}", tmp);
    println!("{:?}", bts);
}
