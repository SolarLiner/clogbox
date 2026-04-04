#[derive(Debug, Copy, Clone)]
pub struct Lcg {
    state: u32,
}

impl Lcg {
    pub const fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    pub const fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state
    }

    pub const fn next_u64(&mut self) -> u64 {
        self.next_u32() as u64 | ((self.next_u32() as u64) << 32)
    }

    pub const fn next_f32(&mut self) -> f32 {
        const FACTOR: f32 = (u32::MAX as f32).recip();
        self.next_u32() as f32 * FACTOR
    }

    pub const fn next_f64(&mut self) -> f64 {
        const FACTOR: f64 = (u64::MAX as f64).recip();
        self.next_u64() as f64 * FACTOR
    }
}
