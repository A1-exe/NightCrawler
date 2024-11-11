
use core::fmt;
use crate::mmu::{Mmu, VirtAddr, Perm, PermBit, MmuError};

// An R-type instruction
#[derive(Debug)]
struct Rtype {
    funct7: u32,
    rs2:    Register,
    rs1:    Register,
    funct3: u32,
    rd:     Register,
}

impl From<u32> for Rtype {
    fn from(inst: u32) -> Self {
        Rtype {
            funct7: (inst >> 25) & 0b1111111,
            rs2:    Register::from((inst >> 20) & 0b11111),
            rs1:    Register::from((inst >> 15) & 0b11111),
            funct3: (inst >> 12) & 0b111,
            rd:     Register::from((inst >>  7) & 0b11111),
        }
    }
}

// An S-type instruction
#[derive(Debug)]
struct Stype {
    imm:    i32,
    rs2:    Register,
    rs1:    Register,
    funct3: u32,
}

impl From<u32> for Stype {
    fn from(inst: u32) -> Self {
        let imm115 = (inst >> 25) & 0b1111111;
        let imm40  = (inst >>  7) & 0b11111;

        let imm = (imm115 << 5) | imm40;
        let imm = ((imm as i32) << 20) >> 20;

        Stype {
            imm:    imm,
            rs2:    Register::from((inst >> 20) & 0b11111),
            rs1:    Register::from((inst >> 15) & 0b11111),
            funct3: (inst >> 12) & 0b111,
        }
    }
}

// A J-type instruction
#[derive(Debug)]
struct Jtype {
    imm: i32,
    rd:  Register,
}

impl From<u32> for Jtype {
    fn from(inst: u32) -> Self {
        let imm20   = (inst >> 31) & 1;
        let imm101  = (inst >> 21) & 0b1111111111;
        let imm11   = (inst >> 20) & 1;
        let imm1912 = (inst >> 12) & 0b11111111;

        let imm = (imm20 << 20) | (imm1912 << 12) | (imm11 << 11) |
            (imm101 << 1);
        let imm = ((imm as i32) << 11) >> 11;

        Jtype {
            imm: imm,
            rd:  Register::from((inst >> 7) & 0b11111),
        }
    }
}

// A B-type instruction
#[derive(Debug)]
struct Btype {
    imm:    i32,
    rs2:    Register,
    rs1:    Register,
    funct3: u32,
}

impl From<u32> for Btype {
    fn from(inst: u32) -> Self {
        let imm12  = (inst >> 31) & 1;
        let imm105 = (inst >> 25) & 0b111111;
        let imm41  = (inst >>  8) & 0b1111;
        let imm11  = (inst >>  7) & 1;

        let imm = (imm12 << 12) | (imm11 << 11) |(imm105 << 5) | (imm41 << 1);
        let imm = ((imm as i32) << 19) >> 19;

        Btype {
            imm:    imm,
            rs2:    Register::from((inst >> 20) & 0b11111),
            rs1:    Register::from((inst >> 15) & 0b11111),
            funct3: (inst >> 12) & 0b111,
        }
    }
}

// An I-type instruction
#[derive(Debug)]
struct Itype {
    imm:    i32,
    rs1:    Register,
    funct3: u32,
    rd:     Register,
}

impl From<u32> for Itype {
    fn from(inst: u32) -> Self {
        Itype {
            imm:    (inst as i32) >> 20,
            rs1:    Register::from((inst >> 15) & 0b11111),
            funct3: (inst >> 12) & 0b111,
            rd:     Register::from((inst >>  7) & 0b11111),
        }
    }
}

#[derive(Debug)]
struct Utype {
    imm: i32,
    rd:  Register,
}

impl From<u32> for Utype {
    fn from(inst: u32) -> Self {
        Utype {
            imm: (inst & !0xfff) as i32,
            rd:  Register::from((inst >> 7) & 0b11111),
        }
    }
}

// x0-x31, pc registers
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Register {
    Zero,
    Ra,
    Sp,
    Gp,
    Tp,
    T0,
    T1,
    T2,
    S0,
    S1,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    T3,
    T4,
    T5,
    T6,
    Pc
}

impl From<u32> for Register {
    fn from(val: u32) -> Self {
        assert!(val < 32); // Don't allow access to pc register
        unsafe {
            core::ptr::read_unaligned(&(val as usize) as *const usize as *const Register)
        }
    }
}

// Utility macros
#[macro_export]
macro_rules! push { // Push value of generic size onto emu stack
    ($emu:ident, $expr:expr) => {
        {
            let sp = $emu.reg(Register::Sp) - (core::mem::size_of_val(&$expr) as u64);
            println!("Pushing 0x{:X} to 0x{:X}", $expr, sp);
            $emu.memory.write(VirtAddr(sp as usize), $expr).expect("push failed");
            $emu.set_reg(Register::Sp, sp);
        }
    }
}

#[allow(unused)]
macro_rules! pop {
    ($generic:ty, $emu:ident) => {
        {
            let sp = emu.reg(Register::Sp);
            let val = emu.memory.read::<$generic>(VirtAddr(sp as usize)).expect("pop failed");
            emu.set_reg(Register::Sp, sp + (core::mem::size_of::<$generic>() as u64));
            val as $generic
        }
    }
}

#[derive(Debug, Clone)]
pub enum FileType {
    FuzzInput,
    Unimplemented
}

#[derive(Debug, Clone)]
pub struct File {
    pub r#type: FileType,

    // File data
    pub data: Option<Vec<u8>>,

    // Cursor for seeking
    pub offset: Option<usize>,
}

impl File {
    pub fn new(r#type: FileType) -> Self {
        match r#type {
            FileType::FuzzInput => {
                Self {
                    r#type,
                    data: Some(Vec::new()),
                    offset: Some(0),
                }
            }
            _ => unimplemented!("Non-fuzz input file types are not yet supported"),
        }
    }
}


#[derive(Debug)]
pub enum EmuExit {
    // Error loading program
    LoadError(String),
    // Exit due to syscall
    Syscall,
    // Error handling syscall,
    SyscallError(String),
    // Clean exit
    Exit,
    // Read/write caused overflow of address space
    AddressOverflow(VirtAddr, usize),
    // Read/write to invalid memory
    InvalidAccess(VirtAddr, usize),
    // Read/write to memory with invalid permissions
    ReadFault(VirtAddr),
    // Read of uninitialized memory
    UninitFault(VirtAddr),
    // Write to non-writable memory
    WriteFault(VirtAddr),
}

impl From<MmuError> for EmuExit {
    fn from(err: MmuError) -> Self {
        match err {
            MmuError::InvalidAddr(addr) => EmuExit::InvalidAccess(addr, 0),
            MmuError::InvalidPerms(Perm(exp_perm), addr, size, Perm(real_perm)) => {
                let read = PermBit::Read as u8;
                let write = PermBit::Write as u8;
                let raw = PermBit::ReadAfterWrite as u8;
                let unknown = PermBit::Unknown as u8;

                // println!("Converting Perms: {:#X} {:#X} {:#X} {:#X}", exp_perm, addr.0, size, real_perm);
                if real_perm == unknown || exp_perm & raw != 0 {
                    EmuExit::UninitFault(addr)
                } else if exp_perm & read == 0 {
                    EmuExit::ReadFault(addr)
                } else if exp_perm & write == 0 {
                    EmuExit::WriteFault(addr)
                } else {
                    panic!("InvalidPerms: {:#X} {:#X} {:#X} {:#X}", exp_perm, addr.0, size, real_perm)
                }
            }
            MmuError::AddressOverflow(addr, size) => EmuExit::AddressOverflow(addr, size),
            MmuError::OutOfMemory(e) => panic!("OutOfMemory: {:?}", e),
            MmuError::MetaError(e) => panic!("MetaError: {:?}", e),
        }
    }
}

// Emulated process/system state
pub struct Emulator {
    pub memory: Mmu,
    registers: [u64; 33],
    pub files: Vec<Option<File>>,

}

impl fmt::Debug for Emulator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emulator")
            .field("zero", &self.reg(Register::Zero))
            .field("ra", &self.reg(Register::Ra))
            .field("sp", &self.reg(Register::Sp))
            .field("gp", &self.reg(Register::Gp))
            .field("tp", &self.reg(Register::Tp))
            .field("t0", &self.reg(Register::T0))
            .field("t1", &self.reg(Register::T1))
            .field("t2", &self.reg(Register::T2))
            .field("s0", &self.reg(Register::S0))
            .field("s1", &self.reg(Register::S1))
            .field("a0", &self.reg(Register::A0))
            .field("a1", &self.reg(Register::A1))
            .field("a2", &self.reg(Register::A2))
            .field("a3", &self.reg(Register::A3))
            .field("a4", &self.reg(Register::A4))
            .field("a5", &self.reg(Register::A5))
            .field("a6", &self.reg(Register::A6))
            .field("a7", &self.reg(Register::A7))
            .field("s2", &self.reg(Register::S2))
            .field("s3", &self.reg(Register::S3))
            .field("s4", &self.reg(Register::S4))
            .field("s5", &self.reg(Register::S5))
            .field("s6", &self.reg(Register::S6))
            .field("s7", &self.reg(Register::S7))
            .field("s8", &self.reg(Register::S8))
            .field("s9", &self.reg(Register::S9))
            .field("s10", &self.reg(Register::S10))
            .field("s11", &self.reg(Register::S11))
            .field("t3", &self.reg(Register::T3))
            .field("t4", &self.reg(Register::T4))
            .field("t5", &self.reg(Register::T5))
            .field("t6", &self.reg(Register::T6))
            .field("pc", &self.reg(Register::Pc))
            .finish()
    }
}

impl Emulator {
    // Create new emulator with `size` bytes of memory
    pub fn new(size: usize) -> Self {
        Self {
            memory: Mmu::new(size),
            registers: [0; 33],
            files: Vec::new(),
        }
    }

    // Fork from the existing emulator
    pub fn fork(&self) -> Self {
        Self {
            memory: self.memory.fork(),
            registers: self.registers,
            files: self.files.clone(),
        }
    }
    
    // Create a new file
    pub fn new_file(&mut self, r#type: FileType) -> usize {
        let file = File::new(r#type);
        self.files.push(Some(file));
        self.files.len() - 1
    }

    // Reset the emulator to the state of another emulator
    pub fn reset(&mut self, other: &Self) {
        self.memory = other.memory.fork();
        self.registers = other.registers;
        self.files = other.files.clone();
    }

    // Load a program into memory
    pub fn load(&mut self, filename: &str) -> Result<VirtAddr, EmuExit> {
        // Parse ELF file for LOAD sections
        let file_contents = std::fs::read(filename).ok()
            .ok_or(EmuExit::LoadError("Failed to read file from disk.".to_string()))?;

        use elf::endian::AnyEndian;
        use elf::ElfBytes;
        let file = ElfBytes::<AnyEndian>::minimal_parse(&file_contents.as_slice()).ok()
            .ok_or(EmuExit::LoadError("Failed to parse ELF file.".to_string()))?;

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
            self.memory.set_perms(virt_addr, mem_size, Perm(PermBit::Write as u8)).ok()
                .ok_or(EmuExit::LoadError("Failed to set permissions on memory.".to_string()))?;

            // Write from file to memory
            self.memory.write_from(virt_addr, &file_contents[file_offset..file_offset.checked_add(file_size).ok_or(EmuExit::LoadError("File offsets caused addr overflow".to_string()))?]).ok()
                .ok_or(EmuExit::LoadError("Failed to write from file to memory".to_string()))?;

            // Write 0 padding if necessary
            if file_size < mem_size {
                self.memory.write_from(
                    VirtAddr(virt_addr.0.checked_add(file_size).ok_or(EmuExit::LoadError("File offsets caused addr overflow".to_string()))?), 
                    &vec![0; mem_size - file_size]).ok()
                .ok_or(EmuExit::LoadError("Failed to write padding to memory".to_string()))?;
            }

            // Set appropriate permissions
            self.memory.set_perms(virt_addr, mem_size, Perm(perms))
                .ok().ok_or(EmuExit::LoadError("Failed to set permissions on memory.".to_string()))?;

            // Move allocator beyond loaded sections
            self.memory.alloc_base = VirtAddr(std::cmp::max(
                self.memory.alloc_base.0, 
                virt_addr.0.checked_add(mem_size + 0xFFF).ok_or(EmuExit::LoadError("File offsets caused addr overflow".to_string()))? & !0xFFF)
            );
        }

        // Return entry point
        Ok(VirtAddr(file.ehdr.e_entry as usize))
    }


    pub fn reg(&self, reg: Register) -> u64 {
        if reg == Register::Zero {
            0
        } else {
            self.registers[reg as usize]
        }
    }

    pub fn set_reg(&mut self, reg: Register, val: u64) {
        // println!("Setting register {:?} to {:#X}", reg, val);
        if reg != Register::Zero {
            self.registers[reg as usize] = val;
        }
    }

    pub fn run(&mut self) -> Result<(), EmuExit> {
        'next_inst: loop {
            // Fetch program counter
            let pc = self.reg(Register::Pc);
            // println!("PC: {:#X}", pc);
            // println!("{:#x?}", self);

            let instr = self.memory.read::<u32>(VirtAddr(pc as usize))?;

            let opcode = instr & 0b1111111;

            // Notes:
            // - Remember to sign extend properly
            match opcode {
                0b0110111 => {
                    // LUI
                    let utype: Utype = instr.into();
                    let imm = utype.imm as i64 as u64;
                    let rd = utype.rd;
                    self.set_reg(rd, imm);
                }
                0b0010111 => {
                    // AUIPC
                    let utype: Utype = instr.into();
                    let imm = utype.imm as i64 as u64;
                    let rd = utype.rd;
                    self.set_reg(rd, pc.wrapping_add(imm));
                }
                0b1101111 => {
                    // JAL
                    let jtype: Jtype = instr.into();
                    let imm = jtype.imm as i64 as u64;
                    let rd = jtype.rd;
                    self.set_reg(rd, pc.wrapping_add(4));
                    self.set_reg(Register::Pc, pc.wrapping_add(imm));
                    continue 'next_inst;
                }
                0b1100111 => {
                    // JALR
                    let itype: Itype = instr.into();
                    match itype.funct3 {
                        0b000 => {
                            let imm = itype.imm as i64 as u64;
                            let rs1 = itype.rs1;
                            let rd = itype.rd;
                            let rs1_val = self.reg(rs1);
                            let target = rs1_val.wrapping_add(imm);
                            // println!("JALR: target = {:#X}", target);
                            self.set_reg(rd, pc.wrapping_add(4));
                            self.set_reg(Register::Pc, target);
                            continue 'next_inst;
                        }
                        _ => unimplemented!("(JALR) Unhandle funct3: {:#03b}", itype.funct3),
                    }
                }
                0b1100011 => {
                    // Branch instructions
                    let btype: Btype = instr.into();
                    let imm = btype.imm as i64 as u64;
                    let rs1 = btype.rs1;
                    let rs2 = btype.rs2;
                    let funct3 = btype.funct3;

                    let rs1_val = self.reg(rs1);
                    let rs2_val = self.reg(rs2);

                    let take_branch = match funct3 {
                        /* BEQ */ 0b000 => rs1_val == rs2_val,
                        /* BNE */ 0b001 => rs1_val != rs2_val,
                        /* BLT */ 0b100 => (rs1_val as i64) < (rs2_val as i64),
                        /* BGE */ 0b101 => (rs1_val as i64) >= (rs2_val as i64),
                        /* BLTU */ 0b110 => rs1_val < rs2_val,
                        /* BGEU */ 0b111 => rs1_val >= rs2_val,
                        _ => unimplemented!("(Branch) Unhandle funct3: {:#03b}", funct3),
                    };

                    if take_branch {
                        self.set_reg(Register::Pc, pc.wrapping_add(imm));
                        continue 'next_inst;
                    }
                }
                0b0000011 => {
                    // Load instructions
                    let itype: Itype = instr.into();
                    let imm = itype.imm as i64 as u64;
                    let rs1 = itype.rs1;
                    let rd = itype.rd;
                    let funct3 = itype.funct3;

                    let rs1_val = self.reg(rs1);
                    let addr = rs1_val.wrapping_add(imm);

                    let val = match funct3 {
                        /* LB */ 0b000 => self.memory.read::<i8>(VirtAddr(addr as usize))? as u64,
                        /* LH */ 0b001 => self.memory.read::<i16>(VirtAddr(addr as usize))? as u64,
                        /* LW */ 0b010 => self.memory.read::<i32>(VirtAddr(addr as usize))? as u64,
                        /* LD */ 0b011 => self.memory.read::<i64>(VirtAddr(addr as usize))? as u64,
                        /* LBU */ 0b100 => self.memory.read::<u8>(VirtAddr(addr as usize))? as u64,
                        /* LHU */ 0b101 => self.memory.read::<u16>(VirtAddr(addr as usize))? as u64,
                        /* LWU */ 0b110 => self.memory.read::<u32>(VirtAddr(addr as usize))? as u64,
                        _ => unimplemented!("(Load) Unhandle funct3: {:#03b}", funct3),
                    };

                    self.set_reg(rd, val);
                }
                0b0100011 => {
                    // Store instructions
                    let stype: Stype = instr.into();
                    let imm = stype.imm as i64 as u64;
                    let rs1 = stype.rs1;
                    let rs2 = stype.rs2;
                    let funct3 = stype.funct3;

                    let rs1_val = self.reg(rs1);
                    let rs2_val = self.reg(rs2);
                    let addr = VirtAddr(rs1_val.wrapping_add(imm) as usize);

                    match funct3 {
                        /* SB */ 0b000 => self.memory.write(addr, rs2_val as u8)?,
                        /* SH */ 0b001 => self.memory.write(addr, rs2_val as u16)?,
                        /* SW */ 0b010 => self.memory.write(addr, rs2_val as u32)?,
                        /* SD */ 0b011 => self.memory.write(addr, rs2_val as u64)?,
                        _ => unimplemented!("(Store) Unhandle funct3: {:#03b}", funct3),
                    };
                }
                0b0010011 => {
                    // ALU instructions
                    let itype: Itype = instr.into();
                    let imm = itype.imm as i64 as u64;
                    let rs1 = itype.rs1;
                    let rd = itype.rd;
                    let funct3 = itype.funct3;

                    let rs1_val = self.reg(rs1);

                    let val = match funct3 {
                        /* ADDI */ 0b000 => rs1_val.wrapping_add(imm),
                        /* SLTI */ 0b010 => ((rs1_val as i64) < (imm as i64)) as i64 as u64,
                        /* SLTIU */ 0b011 => (rs1_val < imm) as u64,
                        /* XORI */ 0b100 => rs1_val ^ imm,
                        /* ORI */ 0b110 => rs1_val | imm,
                        /* ANDI */ 0b111 => rs1_val & imm,
                        /* L-SHIFT */ 0b001 => { 
                            let mode = (imm >> 6) & 0b111111;
                            let shift = imm & 0b111111;
                            match mode {
                                /* SLLI */ 0b000000 => rs1_val << shift,
                                _ => unreachable!("(LSH) Unhandle mode: {:#06b}", mode),
                            }
                        },
                        /* R-SHIFT */ 0b101 => {
                            let mode = (imm >> 6) & 0b111111;
                            let shift = imm & 0b111111;

                            match mode {
                                /* SRLI */ 0b000000 => rs1_val >> shift,
                                /* SRAI */ 0b010000 => ((rs1_val as i64) >> shift) as u64,
                                _ => unreachable!("(RSH) Unhandle mode: {:#06b}", mode),
                            }
                        }
                        _ => unimplemented!("(ALU) Unhandle funct3: {:#03b}", funct3),
                    };

                    self.set_reg(rd, val);
                }
                0b0110011 => {
                    // R-type instructions
                    let rtype: Rtype = instr.into();
                    let rs1 = rtype.rs1;
                    let rs2 = rtype.rs2;
                    let rd = rtype.rd;
                    let funct3 = rtype.funct3;
                    let funct7 = rtype.funct7;

                    let rs1_val = self.reg(rs1);
                    let rs2_val = self.reg(rs2);

                    let val = match funct3 {
                        0b000 => {
                            match funct7 {
                                /* ADD */ 0b0000000 => rs1_val.wrapping_add(rs2_val),
                                /* SUB */ 0b0100000 => rs1_val.wrapping_sub(rs2_val),
                                _ => unreachable!("(ADD/SUB) Unhandle mode: {:#07b}", funct7),
                            }
                        }
                        /* SLL */ 0b001 => rs1_val << (rs2_val & 0b111111),
                        /* SLT */ 0b010 => ((rs1_val as i64) < (rs2_val as i64)) as i64 as u64,
                        /* SLTU */ 0b011 => (rs1_val < rs2_val) as u64,
                        /* XOR */ 0b100 => rs1_val ^ rs2_val,
                        0b101 => {
                            match funct7 {
                                /* SRL */ 0b0000000 => rs1_val >> (rs2_val & 0b111111),
                                /* SRA */ 0b0100000 => ((rs1_val as i64) >> (rs2_val & 0b111111)) as u64,
                                _ => unreachable!("(SRL/SRA) Unhandle mode: {:#07b}", funct7),
                            }
                        }
                        /* OR */ 0b110 => rs1_val | rs2_val,
                        /* AND */ 0b111 => rs1_val & rs2_val,
                        _ => unimplemented!("(R-Type) Unhandle funct3: {:#03b}", funct3),
                    };

                    self.set_reg(rd, val);
                }
                0b0011011 => {
                    // I-type instructions
                    let itype: Itype = instr.into();
                    let imm = itype.imm as i64 as u64;
                    let rs1 = itype.rs1;
                    let rd = itype.rd;
                    let funct3 = itype.funct3;

                    let rs1_val = self.reg(rs1);

                    let val = match funct3 {
                        /* ADDIW */ 0b000 => rs1_val.wrapping_add(imm) as i32 as i64 as u64,
                        /* SLLIW */ 0b001 => rs1_val << (imm & 0b11111) as i32 as i64 as u64,
                        0b101 => {
                            let mode = (imm >> 5) & 0b111111;
                            let shift = imm & 0b11111;
                            match mode {
                                /* SRLIW */ 0b000000 => rs1_val >> shift as i32 as i64 as u64,
                                /* SRAIW */ 0b010000 => ((rs1_val as i32) >> shift) as i64 as u64,
                                _ => unreachable!("(SRLIW/SRAIW) Unhandle mode: {:#06b}", mode),
                            }
                        }
                        _ => unimplemented!("(I-Type) Unhandle funct3: {:#03b}", funct3),
                    };

                    self.set_reg(rd, val);
                }
                0b0111011 => {
                    // R4-type instructions
                    let rtype: Rtype = instr.into();
                    let rs1 = rtype.rs1;
                    let rs2 = rtype.rs2;
                    let rd = rtype.rd;
                    let funct3 = rtype.funct3;
                    let funct7 = rtype.funct7;

                    let rs1_val = self.reg(rs1);
                    let rs2_val = self.reg(rs2);

                    let val = match funct3 {
                        0b000 => {
                            match funct7 {
                                /* ADDW */ 0b0000000 => rs1_val.wrapping_add(rs2_val) as i32 as i64 as u64,
                                /* SUBW */ 0b0100000 => rs1_val.wrapping_sub(rs2_val) as i32 as i64 as u64,
                                _ => unreachable!("(ADDW/SUBW) Unhandle mode: {:#07b}", funct7),
                            }
                        }
                        /* SLLW */ 0b001 => rs1_val << (rs2_val & 0b11111) as i32 as i64 as u64,
                        /* SRLW */ 0b101 => {
                            match funct7 {
                                /* SRLW */ 0b0000000 => rs1_val >> (rs2_val & 0b11111) as i32 as i64 as u64,
                                /* SRAW */ 0b0100000 => ((rs1_val as i32) >> (rs2_val & 0b11111)) as i64 as u64,
                                _ => unreachable!("(SRLW/SRAW) Unhandle mode: {:#07b}", funct7),
                            }
                        }
                        _ => unimplemented!("(R4-Type) Unhandle funct3: {:#03b}", funct3),
                    };

                    self.set_reg(rd, val);
                }
                0b0001111 => {
                    // FENCE
                    let pred = (instr >> 24) & 0b1111;
                    let succ = (instr >> 20) & 0b1111;
                    if pred == 0b0011 && succ == 0b0011 {
                        // FENCE.TSO
                        unimplemented!("(FENCE) FENCE.TSO");
                    }
                    if pred != 0 || succ != 0 {
                        unimplemented!("(FENCE) Unhandle pred/succ: {:#04b} {:#04b}", pred, succ);
                    }
                }
                0b1110011 => {
                    // ECALL/EBREAK
                    let funct3 = (instr >> 12) & 0b111;
                    match funct3 {
                        0b000 => {
                            // ECALL
                            return Err(EmuExit::Syscall);
                        },
                        0b001 => {
                            // EBREAK
                            unimplemented!("EBREAK not implemented");
                        },
                        _ => unimplemented!("(ECALL/EBREAK) Unhandle funct3: {:#03b}", funct3),
                    }
                }
                _ => unimplemented!("Unhandle opcode: {:#09b}", opcode),
            }

            // Increment program counter
            self.set_reg(Register::Pc, pc.wrapping_add(4));
        }
    }
}