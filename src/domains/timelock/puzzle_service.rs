//! Core repeated-squaring cryptographic implementation (RSW-style puzzle).
//! Generates safe blum primes, creates puzzles, and accurately evaluates them.

use std::hint::black_box;
use std::time::{Duration, Instant};

use glass_pumpkin::prime;
use num_bigint::BigUint;
use num_integer::Integer;
use rand::rngs::OsRng;
use rand::RngCore;

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Error, Result};

use super::puzzle_material::{TimelockPuzzleMaterial, FILE_KEY_SIZE};

#[cfg(any(test, debug_assertions))]
const MINIMUM_MODULUS_BITS: usize = 256;
#[cfg(not(any(test, debug_assertions)))]
const MINIMUM_MODULUS_BITS: usize = 2048;

pub fn create_puzzle_and_wrap_key(
    key: &[u8; FILE_KEY_SIZE],
    iterations: u64,
    modulus_bits: usize,
) -> Result<TimelockPuzzleMaterial> {
    if iterations == 0 {
        return Err(Error::InvalidArgument(
            "iterations must be greater than zero".to_string(),
        ));
    }

    if modulus_bits < MINIMUM_MODULUS_BITS || !modulus_bits.is_multiple_of(2) {
        return Err(Error::InvalidArgument(format!(
            "modulus bits must be even and >= {MINIMUM_MODULUS_BITS}"
        )));
    }

    let prime_bits = modulus_bits / 2;
    let p = gen_blum_prime(prime_bits, "p")?;
    let min_prime_diff_bits = (prime_bits as u64) / 2;
    let mut q = gen_blum_prime(prime_bits, "q")?;

    while p == q || too_close(&p, &q, min_prime_diff_bits) {
        q = gen_blum_prime(prime_bits, "q")?;
    }

    let one = BigUint::from(1_u8);

    let n = &p * &q;
    let pm1 = &p - &one;
    let qm1 = &q - &one;
    let lambda = pm1.lcm(&qm1);

    let a = random_coprime_base(&n, modulus_bits);

    let exponent = BigUint::from(2_u8).modpow(&BigUint::from(iterations), &lambda);
    let b = a.modpow(&exponent, &n);

    let mask = derive_key_mask(&b);
    let wrapped_key = xor_32(key, &mask);

    Ok(TimelockPuzzleMaterial {
        modulus_n: n,
        base_a: a,
        wrapped_key,
        iterations,
        modulus_bits: modulus_bits as u16,
    })
}

fn gen_blum_prime(bits: usize, name: &str) -> Result<BigUint> {
    let four = BigUint::from(4_u8);
    let three = BigUint::from(3_u8);

    loop {
        let candidate = prime::new(bits)
            .map_err(|err| Error::Crypto(format!("failed to generate prime {name}: {err}")))?;
        if (&candidate % &four) == three {
            return Ok(candidate);
        }
    }
}

fn too_close(p: &BigUint, q: &BigUint, min_diff_bits: u64) -> bool {
    let (a, b) = if p > q { (p, q) } else { (q, p) };
    let diff = a - b;
    diff.bits() < min_diff_bits
}

fn random_coprime_base(modulus_n: &BigUint, modulus_bits: usize) -> BigUint {
    let one = BigUint::from(1_u8);
    let two = BigUint::from(2_u8);
    let bytes_len = modulus_bits.div_ceil(8).max(1);
    let mut bytes = vec![0_u8; bytes_len];
    let mut rng = OsRng;

    loop {
        rng.fill_bytes(&mut bytes);
        let candidate = BigUint::from_bytes_be(&bytes) % modulus_n;

        if candidate < two {
            continue;
        }

        if candidate.gcd(modulus_n) == one {
            return candidate;
        }
    }
}

pub fn unwrap_key(
    material: &TimelockPuzzleMaterial,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
) -> Result<[u8; FILE_KEY_SIZE]> {
    unwrap_key_with_cancel(material, on_progress, None)
}

pub fn unwrap_key_with_cancel(
    material: &TimelockPuzzleMaterial,
    mut on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<[u8; FILE_KEY_SIZE]> {
    if is_cancelled(cancellation) {
        return Err(Error::Cancelled);
    }

    let mut value = material.base_a.clone();
    let started = Instant::now();

    for i in 0..material.iterations {
        value = repeated_square_step(value, &material.modulus_n);

        let completed = i + 1;
        if completed % 1_024 == 0 && is_cancelled(cancellation) {
            return Err(Error::Cancelled);
        }

        let should_emit = completed == material.iterations || completed % 10_000 == 0;
        if should_emit {
            if is_cancelled(cancellation) {
                return Err(Error::Cancelled);
            }

            let elapsed_secs = started.elapsed().as_secs_f64().max(0.000_001);
            let rate = completed as f64 / elapsed_secs;
            let remaining = material.iterations.saturating_sub(completed);
            let eta = if rate <= 0.0 {
                None
            } else {
                Some((remaining as f64 / rate).ceil() as u64)
            };

            if let Some(handler) = on_progress.as_deref_mut() {
                handler(ProgressStatus::new(
                    "unlock-timelock",
                    completed,
                    material.iterations,
                    eta,
                    Some(rate),
                ));
            }
        }
    }

    let mask = derive_key_mask(&value);
    Ok(xor_32(&material.wrapped_key, &mask))
}

pub fn benchmark_repeated_squaring_iterations(duration: Duration) -> u64 {
    let modulus = calibration_modulus();
    let mut value = calibration_base();
    let started = Instant::now();
    let mut iterations = 0_u64;

    while started.elapsed() < duration {
        value = repeated_square_step(value, &modulus);
        iterations = iterations.saturating_add(1);
    }

    black_box(value);
    iterations
}

pub fn benchmark_repeated_squaring_iterations_per_second(duration: Duration) -> u64 {
    let modulus = calibration_modulus();
    let mut value = calibration_base();

    warm_up_repeated_squaring(&mut value, &modulus, duration / 5);

    let started = Instant::now();
    let mut iterations = 0_u64;

    while started.elapsed() < duration {
        value = repeated_square_step(value, &modulus);
        iterations = iterations.saturating_add(1);
    }

    let elapsed = started.elapsed();
    black_box(value);

    calculate_iterations_per_second(iterations, elapsed)
}

fn is_cancelled(cancellation: Option<&CancellationToken>) -> bool {
    cancellation.is_some_and(|token| token.is_cancelled())
}

fn repeated_square_step(value: BigUint, modulus_n: &BigUint) -> BigUint {
    (&value * &value) % modulus_n
}

fn warm_up_repeated_squaring(value: &mut BigUint, modulus_n: &BigUint, duration: Duration) {
    let started = Instant::now();

    while started.elapsed() < duration {
        *value = repeated_square_step(std::mem::take(value), modulus_n);
    }
}

fn calculate_iterations_per_second(iterations: u64, elapsed: Duration) -> u64 {
    if iterations == 0 {
        return 0;
    }

    let elapsed_nanos = elapsed.as_nanos();
    if elapsed_nanos == 0 {
        return iterations;
    }

    let scaled = (iterations as u128)
        .saturating_mul(1_000_000_000)
        .saturating_add(elapsed_nanos / 2)
        / elapsed_nanos;

    scaled.min(u64::MAX as u128) as u64
}

fn calibration_modulus() -> BigUint {
    (BigUint::from(1_u8) << 2048_usize) - BigUint::from(159_u16)
}

fn calibration_base() -> BigUint {
    (BigUint::from(1_u8) << 1024_usize) + BigUint::from(65_537_u32)
}

fn derive_key_mask(result: &BigUint) -> [u8; FILE_KEY_SIZE] {
    let digest = blake3::hash(&result.to_bytes_be());
    let mut out = [0_u8; FILE_KEY_SIZE];
    out.copy_from_slice(digest.as_bytes());
    out
}

fn xor_32(left: &[u8; FILE_KEY_SIZE], right: &[u8; FILE_KEY_SIZE]) -> [u8; FILE_KEY_SIZE] {
    let mut out = [0_u8; FILE_KEY_SIZE];
    for i in 0..FILE_KEY_SIZE {
        out[i] = left[i] ^ right[i];
    }
    out
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use num_integer::Integer;

    use crate::base::Error;

    use std::time::Duration;

    use super::{
        benchmark_repeated_squaring_iterations, benchmark_repeated_squaring_iterations_per_second,
        calculate_iterations_per_second, create_puzzle_and_wrap_key, too_close, unwrap_key,
    };

    #[test]
    fn wraps_and_unwraps_key() {
        let key = [42_u8; 32];
        let puzzle = create_puzzle_and_wrap_key(&key, 64, 256).expect("must create puzzle");
        let recovered = unwrap_key(&puzzle, None).expect("must unwrap key");
        assert_eq!(recovered, key);
        assert!(puzzle.base_a > BigUint::from(1_u8));
        assert!(puzzle.base_a < puzzle.modulus_n);
        assert_eq!(puzzle.base_a.gcd(&puzzle.modulus_n), BigUint::from(1_u8));
    }

    #[test]
    fn rejects_zero_iterations() {
        let key = [1_u8; 32];
        let err = create_puzzle_and_wrap_key(&key, 0, 256).expect_err("must fail");

        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(err
            .to_string()
            .contains("iterations must be greater than zero"));
    }

    #[test]
    fn rejects_invalid_modulus_bits() {
        let key = [1_u8; 32];

        let too_small = create_puzzle_and_wrap_key(&key, 1, 128).expect_err("must fail");
        assert!(matches!(too_small, Error::InvalidArgument(_)));

        let odd_size = create_puzzle_and_wrap_key(&key, 1, 257).expect_err("must fail");
        assert!(matches!(odd_size, Error::InvalidArgument(_)));
    }

    #[test]
    fn emits_progress_on_completion() {
        let key = [7_u8; 32];
        let puzzle = create_puzzle_and_wrap_key(&key, 16, 256).expect("must create puzzle");

        let mut events = Vec::new();
        {
            let mut on_progress = |event| events.push(event);
            let recovered = unwrap_key(&puzzle, Some(&mut on_progress)).expect("must unwrap");
            assert_eq!(recovered, key);
        }

        assert!(!events.is_empty());
        let last = events.last().expect("last progress event");
        assert_eq!(last.phase, "unlock-timelock");
        assert_eq!(last.current, puzzle.iterations);
        assert_eq!(last.total, puzzle.iterations);
    }

    #[test]
    fn detects_when_primes_are_too_close() {
        let p = BigUint::from(503_u32);
        let q = BigUint::from(499_u32);

        assert!(too_close(&p, &q, 4));
        assert!(!too_close(&p, &q, 3));
    }

    #[test]
    fn benchmark_reports_positive_iterations() {
        let iterations = benchmark_repeated_squaring_iterations(Duration::from_millis(1));
        assert!(iterations > 0);
    }

    #[test]
    fn benchmark_reports_positive_rate() {
        let rate = benchmark_repeated_squaring_iterations_per_second(Duration::from_millis(5));
        assert!(rate > 0);
    }

    #[test]
    fn calculates_iterations_per_second_from_elapsed_time() {
        assert_eq!(
            calculate_iterations_per_second(250, Duration::from_millis(500)),
            500
        );
        assert_eq!(calculate_iterations_per_second(1, Duration::ZERO), 1);
        assert_eq!(
            calculate_iterations_per_second(0, Duration::from_secs(1)),
            0
        );
    }
}
