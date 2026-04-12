//! Defines the core mathematical material (modulus, base, wrapped key)
//! required to represent or solve a repeated-squaring timelock puzzle.

use num_bigint::BigUint;

pub const FILE_KEY_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct TimelockPuzzleMaterial {
    pub modulus_n: BigUint,
    pub base_a: BigUint,
    pub wrapped_key: [u8; FILE_KEY_SIZE],
    pub iterations: u64,
    pub modulus_bits: u16,
}
