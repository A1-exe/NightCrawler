pub mod emu;
pub mod mmu;

use emu::Emulator;

use std::time::Instant;

fn main() {
    let mut emu = Emulator::new(1024 * 1024);
    let entry_point = emu.load("./test_app")
        .expect("Could not load program");

    println!("Program loaded...");
    println!("Entry point: 0x{:X}", entry_point.0);
    
    
    let mut tmp = [0u8; 4];
    emu.memory.read_into(entry_point, &mut tmp).expect("read failed");
    
    println!("First 4 bytes at entry point: {:X?}", tmp);
    
    // let tmp = emu.memory.alloc(6).unwrap();
    // let forked = emu.fork();
    // let mut total_cases = 0u64;
    // let start = Instant::now();

    // for case in 0..100_000_000 {
    //     emu.memory.write_from(tmp, b"meeper").expect("write failed");
    //     emu.reset(&forked);
    //     total_cases = total_cases.wrapping_add(1);

    //     if total_cases % 10_000 == 0 {
    //         let elapsed = start.elapsed().as_secs_f64();
    //         println!("[{:10.6}] cases {:10} | fcps {:10.2}", elapsed, total_cases, total_cases as f64 / elapsed);
    //     }
    // }
}
