pub mod emu;
pub mod mmu;

use emu::{EmuExit, Emulator, FileType, Register};
use mmu::{Perm, PermBit, VirtAddr};
use nightcrawler::{rdstc, set_thread_affinity};

use std::{io::Write, sync::{Arc, Mutex}, time::Duration};

#[allow(unused)]
use std::time::Instant;

#[macro_export]
macro_rules! pause {
    () => {
        std::io::stdin().read_line(&mut String::new()).expect("Failed to read line");
    };
}

// Track input corpus in memory
struct Corpus {
    inputs: Vec<Vec<u8>>,
}

// Mutate an input
enum Mutator {
    Unimplemented
}

#[derive(Default)]
struct FuzzStats {
    // Total caces
    cases: u64,
    
    // Total crashes
    crashes: u64,

    // Total cycles
    cycles: u64,
    
    // Total cycles during reset
    reset_cycles: u64,

    // Total cycles during run
    run_cycles: u64,

    // Total instructions
    instrs: u64,
}

const STATS_INTERVAL: u64 = 1; // In cases
const NUMBER_OF_WORKERS: usize = 2; // In cores

fn main() {
    let mut emu = Emulator::new(32 * 1024 * 1024);
    let entry_point = emu.load("./test_overflow")
        .expect("Could not load program");
    
    println!("Program loaded...");
    println!("Entry point: 0x{:X}", entry_point.0);
    emu.set_reg(Register::Pc, entry_point.0 as u64);

    // Allocate stack
    let stack_size = 32 * 1024;
    let stack = emu.memory.alloc(stack_size).expect("Stack alloc failed");
    emu.set_reg(Register::Sp, stack.0 as u64 + stack_size as u64);

    let progname = emu.memory.alloc(0x1000).expect("Progname alloc failed");
    emu.memory.write_from(progname, b"test_overflow\0").expect("Failed to write progname");

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

    let arc_emu = Arc::new(emu);
    let arc_corpus = Arc::new(Corpus {
        inputs: Vec::new()
    });
    let arc_stats = Arc::new(Mutex::new(FuzzStats::default()));

    for thread_id in 0..NUMBER_OF_WORKERS {
        let emu = arc_emu.clone();
        let corpus = arc_corpus.clone();
        let stats = arc_stats.clone();

        std::thread::spawn(move || {
            worker(thread_id, emu, corpus, stats);
        });
    }

    let start = Instant::now();
    loop {
        // Output stats 
        std::thread::sleep(std::time::Duration::from_millis(1000));

        let stats = arc_stats.lock().unwrap();
        let elapsed = start.elapsed().as_secs_f64();

        let cases_per = stats.cases as f64 / elapsed;
        let total_cycles = stats.cycles as f64;
        let reset_cycles_per = stats.reset_cycles as f64 / total_cycles;
        let run_cycles_per = stats.run_cycles as f64 / total_cycles;
        let instrs_per = stats.instrs as f64 / elapsed;
        
        println!("[{:10.6}] cases {:6} | crashes {:6} \
            | fcps {:8.2} | reset {:6.2} | run {:6.2} \
            | {:8.2} inst/s", 
            elapsed, stats.cases, stats.crashes, 
            cases_per, reset_cycles_per, run_cycles_per,
            instrs_per
        );
    }
}

fn worker(thread_id: usize, base_emu: Arc<Emulator>, corpus: Arc<Corpus>, stats: Arc<Mutex<FuzzStats>>) {
    // Pin thread to core
    set_thread_affinity(thread_id)
        .expect(&format!("Failed to set thread affinity. TID: {:?}", thread_id));

    // Copy the real 
    let mut fuzz_case = base_emu.fork();

    // Use seeded RNG for testing
    let mut rng = nightcrawler::Rng::new_with_seed(rdstc());

    let mut local_stats = FuzzStats::default();

    println!("Worker {} started", thread_id);

    // Begin fuzzing
    let mut batch_start = rdstc();
    loop {
        // Setup file for fuzzing stdin
        // This is the first file descriptor, so it will be stdin
        let fd = fuzz_case.new_file(FileType::FuzzInput);
        // println!("Fuzzing with fd: {}", fd);

        // Set actual fuzzing data
        if let Some(Some(file)) = fuzz_case.files.get_mut(fd) {
            // We don't have a valid corpus management system yet
            // We will just generate random bytes of random length for now
            let len = rng.rng() % 100;
            let mut data = Vec::with_capacity(len);
            for _ in 0..len {
                data.push(rng.rng() as u8);
            }

            // Hardcoded testcase for testing
            // let mut data = Vec::new();
            // data.extend_from_slice(&[0x41, 0x42, 0x43, 0x44, 0x45, 0x46]);

            file.data = Some(data);
        } else {
            panic!("Failed to set up fuzzing input");
        }

        // Run testcase until completion
        let run_start = rdstc();
        loop {
            let vmexit = fuzz_case.run(&mut local_stats).expect_err("Failed to run emulator");

            match vmexit {
                EmuExit::Syscall => {
                    if let Err(e) = handle_sys(&mut fuzz_case) {
                        match e {
                            EmuExit::Exit => {
                                // println!("Program exited with code: {}", emu.reg(Register::A0));
                            },
                            _ => println!("Syscall error: {:?}", e)
                        };
                        break
                    }
                },
                EmuExit::Exit => {
                    println!("Emulator Exited");
                    break;
                },
                EmuExit::ReadFault(addr) => {
                    // println!("Input caused crash!");
                    // println!("{:#x?}", fuzz_case);
                    // println!("Read fault at: 0x{:X}", addr.0);
                    local_stats.crashes = local_stats.crashes.wrapping_add(1);
                    break;
                },
                EmuExit::WriteFault(addr) => {
                    // println!("Input caused crash!");
                    // println!("{:#x?}", fuzz_case);
                    // println!("Write fault at: 0x{:X}", addr.0);
                    local_stats.crashes = local_stats.crashes.wrapping_add(1);
                    break;
                },
                EmuExit::UninitFault(addr) => {
                    // println!("Input caused crash!");
                    // println!("{:#x?}", fuzz_case);
                    // println!("Uninit fault at: 0x{:X}", addr.0);
                    local_stats.crashes = local_stats.crashes.wrapping_add(1);
                    break;
                },
                _ => {
                    println!("Unexpected vmexit: {:?}", vmexit);
                    break;
                }
            }
        }
        local_stats.run_cycles += rdstc() - run_start;
        
        // Reset emulator for next testcase
        let reset_start = rdstc();
        fuzz_case.reset(&base_emu);
        local_stats.reset_cycles += rdstc() - reset_start;
        
        local_stats.cases = local_stats.cases.wrapping_add(1);
        
        // Report stats periodically
        if (local_stats.cases % STATS_INTERVAL) == 0 {
            let mut stats = stats.lock().unwrap();

            stats.cases += local_stats.cases;
            stats.crashes += local_stats.crashes;
            stats.reset_cycles += local_stats.reset_cycles;
            stats.run_cycles += local_stats.run_cycles;
            stats.instrs += local_stats.instrs;

            let batch_end = rdstc();
            stats.cycles += batch_end - batch_start;
            batch_start = batch_end;

            // Reset local stats structure
            local_stats = FuzzStats::default();
        }
    }
}

fn handle_sys(emu: &mut Emulator) -> Result<(), EmuExit> {
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
            let fd: i32 = emu.reg(Register::A0) as i32;
            if (fd != 1) && (fd != 2) {
                // Only support stdout and stderr for now
                panic!("Invalid file descriptor: {}", fd);
            }

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
        63 => {
            // read
            let fd: usize = emu.reg(Register::A0) as usize;
            let buf: usize = emu.reg(Register::A1) as usize;
            let count: usize = emu.reg(Register::A2) as usize;

            if fd != 0 {
                // Only support stdin
                return Err(EmuExit::SyscallError(format!("Invalid file descriptor: {}", fd)));
            }

            
            if let Some(Some(file)) = emu.files.get(fd) {
                let data = file.data.as_ref().unwrap();
                let cursor = file.offset.unwrap_or(0);
                let cursor_end = core::cmp::min(
                    cursor.saturating_add(count),
                    data.len()
                );

                // Write data from stdin file to buf
                emu.memory.write_from(VirtAddr(buf), &data[cursor..cursor_end]).ok()
                    .ok_or(EmuExit::SyscallError("Failed to write data".to_string()))?;
            } else {
                return Err(EmuExit::SyscallError(format!("Invalid file descriptor: {}", fd)));
            }
        }
        93 | 94 => {
            // exit | exit_group
            return Err(EmuExit::Exit);
        }
        _ => {
            return Err(EmuExit::SyscallError(format!("Unimplemented syscall: {}", syscall)))
        }
    }

    emu.set_reg(Register::Pc, pc.wrapping_add(4));
    Ok(())
}