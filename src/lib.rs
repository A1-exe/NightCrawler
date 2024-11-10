
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
