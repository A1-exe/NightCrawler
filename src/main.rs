pub mod emu;
pub mod mmu;

use emu::{EmuExit, Emulator, Register};
use mmu::{Perm, PermBit, VirtAddr};

use std::io::Write;
#[allow(unused)]
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
    emu.set_reg(Register::Pc, entry_point.0 as u64);

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

    loop {
        let vmexit = emu.run().expect_err("Failed to run emulator");
        match vmexit {
            EmuExit::Syscall => {
                let pc = emu.reg(Register::Pc);
                let syscall = emu.reg(Register::A7);

                match syscall {
                    96 => {
                        // set_tid_address
                        emu.set_reg(Register::A0, 1337);
                    }
                    29 => {
                        // ioctl
                        emu.set_reg(Register::A0, !0);
                    }
                    66 => {
                        // writev
                        let fd = emu.reg(Register::A0) as i32;
                        let iov = emu.reg(Register::A1) as usize;
                        let iovcnt = emu.reg(Register::A2) as usize;

                        let mut total_written = 0usize;
                        for i in 0..iovcnt {
                           let ptr = i.checked_mul(16)
                            .and_then(|v| iov.checked_add(v))
                            .expect("IntOverflow");

                            let buf = emu.memory.read::<u64>(VirtAddr(ptr))
                                .expect("Failed to read iov");
                            let len = emu.memory.read::<u64>(VirtAddr(ptr + 8))
                                .expect("Failed to read iov");

                            // println!("Buffer: 0x{:X} | Len: 0x{:X}", buf, len);

                            let data = emu.memory.peek(VirtAddr(buf as usize), len as usize, Perm(PermBit::Read as u8))
                                .expect("Failed to read data");

                            let written = std::io::stdout().write(data)
                                .expect("Failed to write data");

                            total_written += written;
                        }

                        emu.set_reg(Register::A0, total_written as u64);
                    }
                    93 | 94 => {
                        // exit
                        println!("Program exited with code: {}", emu.reg(Register::A0));
                        // EmuExit::Exit
                        break;
                    }
                    _ => {
                        panic!("Unknown syscall: {}", syscall);
                    }
                }

                emu.set_reg(Register::Pc, pc.wrapping_add(4));
            },
            EmuExit::Exit => {
                println!("Emulator Exited");
                break;
            },
            _ => {
                println!("Unknown vmexit: {:?}", vmexit);
                break;
            }
        }
    }

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
