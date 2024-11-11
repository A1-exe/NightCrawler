
// Random number generator using xorshift algorithm
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Self {
        Self(0xA1E57899214DEE1A)
    }

    pub fn new_with_seed(seed: u64) -> Self {
        Self(seed)
    }

    pub fn rng(&mut self) -> usize {
        self.0 ^= self.0 << 3;
        self.0 ^= self.0 >> 5;
        self.0 ^= self.0 << 21;
        self.0 as usize
    }
}

// Generic support for safe primitive types
pub unsafe trait Prim: Default + Clone + Copy {}

unsafe impl Prim for u8    {}
unsafe impl Prim for u16   {}
unsafe impl Prim for u32   {}
unsafe impl Prim for u64   {}
unsafe impl Prim for u128  {}
unsafe impl Prim for usize {}
unsafe impl Prim for i8    {}
unsafe impl Prim for i16   {}
unsafe impl Prim for i32   {}
unsafe impl Prim for i64   {}
unsafe impl Prim for i128  {}
unsafe impl Prim for isize {}

// OS-agnostic rdstc
pub fn rdstc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe { std::arch::x86_64::_rdtsc() }

    #[cfg(not(target_arch = "x86_64"))]
    {
        unimplemented!("rdstc is only supported on x86_64");
    }
}

// OS-agnostic set affinity
#[cfg(unix)]
pub fn set_thread_affinity(core: usize) -> Result<(), ()> {
    extern "system" {
        fn sched_setaffinity(pid: usize, cpusetsize: usize, mask: *const usize) -> i32;
    }

    const USIZE_BITS: usize = core::mem::size_of::<usize>() * 8;

    let mut mask = [0usize; 32];
    mask[core / USIZE_BITS] |= 1 << (core % USIZE_BITS);

    unsafe {
        if sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(windows)]
pub fn set_thread_affinity(core: usize) -> Result<(), ()> {
    extern "system" {
        fn GetCurrentThread() -> usize;
        fn SetThreadAffinityMask(hThread: usize, dwThreadAffinityMask: usize) -> usize;
    }

    assert!(core < 64, "Windows only supports 64 cores");

    unsafe {
        if SetThreadAffinityMask(GetCurrentThread(), 1usize << core) != 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rng() {
        let mut rng = Rng::new();
        assert_eq!(rng.rng(), 0x9DED86C3EDFB8A3C);
        assert_eq!(rng.rng(), 0xDA597B86C676E502);
        assert_eq!(rng.rng(), 0x59787F751D2FC37A);
        assert_eq!(rng.rng(), 0x894D2691D613566F);
    }
}
