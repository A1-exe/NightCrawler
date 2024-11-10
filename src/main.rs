pub mod emu;
pub mod mmu;

use emu::{Emulator, Register};
use mmu::VirtAddr;

use std::time::Instant;

#[macro_export]
macro_rules! pause {
    () => {
        std::io::stdin().read_line(&mut String::new()).expect("Failed to read line");
    };
}

fn main() {
    let mut emu = Emulator::new(32 * 1024 * 1024);
    let entry_point = emu.load("./test_app")
        .expect("Could not load program");

    println!("Program loaded...");
    println!("Entry point: 0x{:X}", entry_point.0);

    // Allocate stack
    let stack_size = 32 * 1024;
    let stack = emu.memory.alloc(stack_size).expect("Stack alloc failed");
    emu.set_reg(Register::Sp, stack.0 as u64 + stack_size as u64);

    let progname = emu.memory.alloc(0x1000).expect("Progname alloc failed");
    emu.memory.write_from(progname, b"test_app\0").expect("Failed to write progname");

    // Setup start frame
    push!(emu, 0u64); // Auxp
    push!(emu, 0u64); // Envp
    push!(emu, 0u64); // Argv end
    push!(emu, progname.0); // Argv
    push!(emu, 1u64); // Argc

    // // Check proper stack configuration
    // let mut tmp = [0u8; 48];
    // let mut ntmp = unsafe { 
    //     std::slice::from_raw_parts(&tmp as *const u8 as *const u64, 5) 
    // };
    // emu.memory.read_into(VirtAddr(emu.reg(Register::Sp) as usize), &mut tmp).expect("Failed to write tmp");
    // println!("Reading from top of stack: {:#X?}", ntmp);
    // pause!();

    emu.run(Some(entry_point)).expect("Run exited");

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
