/// Small specified PRNG: repeatability is independent of external crate versions.
pub(super) struct Random(pub(super) u64);
impl Random {
    /// Produces the next deterministic xorshift64 value normalized to `[0, 1)`.
    pub(super) fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / ((1_u64 << 53) as f64)
    }
    /// Scales the next random value to the half-open interval `[a, b)`.
    pub(super) fn between(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }
}
