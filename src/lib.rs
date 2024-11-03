
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
pub unsafe trait Primitive: Default + Clone + Copy {}

unsafe impl Primitive for u8    {}
unsafe impl Primitive for u16   {}
unsafe impl Primitive for u32   {}
unsafe impl Primitive for u64   {}
unsafe impl Primitive for u128  {}
unsafe impl Primitive for usize {}
unsafe impl Primitive for i8    {}
unsafe impl Primitive for i16   {}
unsafe impl Primitive for i32   {}
unsafe impl Primitive for i64   {}
unsafe impl Primitive for i128  {}
unsafe impl Primitive for isize {}


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
