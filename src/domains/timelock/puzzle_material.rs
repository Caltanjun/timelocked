//! Defines the core mathematical material (modulus, base, wrapped key)
//! required to represent or solve a repeated-squaring timelock puzzle.

use num_bigint::BigUint;
use std::fmt;

pub const FILE_KEY_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct TimelockPuzzleMaterial {
    pub modulus_n: BigUint,
    pub base_a: BigUint,
    pub wrapped_key: [u8; FILE_KEY_SIZE],
    pub iterations: u64,
    pub modulus_bits: u16,
}

#[derive(Clone)]
pub struct CreatedTimelockPuzzle {
    pub modulus_n: BigUint,
    pub base_a: BigUint,
    pub iterations: u64,
    pub modulus_bits: u16,
    pub solution_mask: [u8; FILE_KEY_SIZE],
}

impl fmt::Debug for CreatedTimelockPuzzle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreatedTimelockPuzzle")
            .field("modulus_n", &self.modulus_n)
            .field("base_a", &self.base_a)
            .field("iterations", &self.iterations)
            .field("modulus_bits", &self.modulus_bits)
            .field("solution_mask", &"[redacted]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_timelock_puzzle_debug_redacts_solution_mask() {
        let puzzle = CreatedTimelockPuzzle {
            modulus_n: BigUint::from(33_u8),
            base_a: BigUint::from(5_u8),
            iterations: 12,
            modulus_bits: 256,
            solution_mask: [222_u8; FILE_KEY_SIZE],
        };

        let formatted = format!("{puzzle:?}");

        assert!(formatted.contains("solution_mask"));
        assert!(formatted.contains("[redacted]"));
        assert!(!formatted.contains("222"));
    }
}
