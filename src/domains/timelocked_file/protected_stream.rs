//! Chunked AEAD encryption and decryption for the protected stream layer.

use std::io::{Read, Write};
use std::time::Instant;

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Error, Result};

use super::chunk_frame::validate_chunk_frame;
use super::{build_chunk_aad, read_chunk_frame, write_chunk_frame, ChunkFrameHeader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkEncryptionStats {
    pub chunk_count: u64,
    pub plaintext_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkDecryptionStats {
    pub chunk_count: u64,
    pub plaintext_bytes: u64,
}

pub fn encrypt_protected_stream(
    reader: &mut impl Read,
    writer: &mut impl Write,
    key_bytes: &[u8; 32],
    superblock_digest: &[u8; 32],
    chunk_size: usize,
    total_plaintext: u64,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
) -> Result<ChunkEncryptionStats> {
    encrypt_protected_stream_with_cancel(
        reader,
        writer,
        key_bytes,
        superblock_digest,
        chunk_size,
        total_plaintext,
        on_progress,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encrypt_protected_stream_with_cancel(
    reader: &mut impl Read,
    writer: &mut impl Write,
    key_bytes: &[u8; 32],
    superblock_digest: &[u8; 32],
    chunk_size: usize,
    total_plaintext: u64,
    mut on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<ChunkEncryptionStats> {
    if chunk_size == 0 {
        return Err(Error::InvalidArgument(
            "chunk_size must be greater than 0".to_string(),
        ));
    }
    if is_cancelled(cancellation) {
        return Err(Error::Cancelled);
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key_bytes));
    let started = Instant::now();
    let mut expected_index = 0_u64;
    let mut plaintext_written = 0_u64;

    let mut current_chunk = vec![0_u8; chunk_size];
    let mut current_len = reader.read(&mut current_chunk)?;

    if current_len == 0 {
        let nonce = random_nonce();
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &[],
                    aad: &build_chunk_aad(superblock_digest, 0, 0, true),
                },
            )
            .map_err(|_| Error::Crypto("chunk encryption failed".to_string()))?;
        write_chunk_frame(
            writer,
            &ChunkFrameHeader {
                index: 0,
                is_last: true,
                plaintext_len: 0,
                ciphertext_len: ciphertext.len() as u32,
                nonce,
            },
            &ciphertext,
        )?;
        emit_progress(
            &mut on_progress,
            started,
            "lock-encrypt",
            0,
            total_plaintext,
        );
        return Ok(ChunkEncryptionStats {
            chunk_count: 1,
            plaintext_bytes: 0,
        });
    }

    loop {
        if is_cancelled(cancellation) {
            return Err(Error::Cancelled);
        }

        let mut next_chunk = vec![0_u8; chunk_size];
        let next_len = reader.read(&mut next_chunk)?;
        let is_last = next_len == 0;
        let plaintext = &current_chunk[..current_len];
        let nonce = random_nonce();
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &build_chunk_aad(
                        superblock_digest,
                        expected_index,
                        current_len as u32,
                        is_last,
                    ),
                },
            )
            .map_err(|_| Error::Crypto("chunk encryption failed".to_string()))?;

        write_chunk_frame(
            writer,
            &ChunkFrameHeader {
                index: expected_index,
                is_last,
                plaintext_len: current_len as u32,
                ciphertext_len: ciphertext.len() as u32,
                nonce,
            },
            &ciphertext,
        )?;

        expected_index += 1;
        plaintext_written += current_len as u64;
        emit_progress(
            &mut on_progress,
            started,
            "lock-encrypt",
            plaintext_written,
            total_plaintext,
        );

        if is_last {
            break;
        }

        current_chunk = next_chunk;
        current_len = next_len;
    }

    Ok(ChunkEncryptionStats {
        chunk_count: expected_index,
        plaintext_bytes: plaintext_written,
    })
}

pub fn decrypt_protected_stream_to_writer(
    reader: &mut impl Read,
    writer: &mut impl Write,
    key_bytes: &[u8; 32],
    superblock_digest: &[u8; 32],
    chunk_size: u32,
    expected_plaintext: u64,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
) -> Result<ChunkDecryptionStats> {
    decrypt_protected_stream_to_writer_with_cancel(
        reader,
        writer,
        key_bytes,
        superblock_digest,
        chunk_size,
        expected_plaintext,
        on_progress,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn decrypt_protected_stream_to_writer_with_cancel(
    reader: &mut impl Read,
    writer: &mut impl Write,
    key_bytes: &[u8; 32],
    superblock_digest: &[u8; 32],
    chunk_size: u32,
    expected_plaintext: u64,
    mut on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<ChunkDecryptionStats> {
    if is_cancelled(cancellation) {
        return Err(Error::Cancelled);
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key_bytes));
    let started = Instant::now();
    let mut expected_index = 0_u64;
    let mut plaintext_written = 0_u64;
    let mut saw_last = false;

    loop {
        if is_cancelled(cancellation) {
            return Err(Error::Cancelled);
        }

        let maybe = read_chunk_frame(reader)?;
        let Some((frame, ciphertext)) = maybe else {
            if expected_index == 0 {
                return Err(Error::InvalidFormat("missing chunk frames".to_string()));
            }
            break;
        };

        validate_protected_stream_frame(&frame, ciphertext.len(), chunk_size, expected_index)?;

        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&frame.nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &build_chunk_aad(
                        superblock_digest,
                        frame.index,
                        frame.plaintext_len,
                        frame.is_last,
                    ),
                },
            )
            .map_err(|_| {
                Error::Crypto(
                    "payload authentication failed (file is corrupted or tampered)".to_string(),
                )
            })?;

        if plaintext.len() != frame.plaintext_len as usize {
            return Err(Error::InvalidFormat(format!(
                "chunk {} plaintext length mismatch",
                frame.index
            )));
        }

        writer.write_all(&plaintext)?;
        plaintext_written += plaintext.len() as u64;
        emit_progress(
            &mut on_progress,
            started,
            "unlock-decrypt",
            plaintext_written,
            expected_plaintext,
        );

        expected_index += 1;
        if frame.is_last {
            saw_last = true;
            break;
        }
    }

    if !saw_last {
        return Err(Error::InvalidFormat(
            "missing terminal chunk marker".to_string(),
        ));
    }

    if read_chunk_frame(reader)?.is_some() {
        return Err(Error::InvalidFormat(
            "unexpected trailing chunk data after terminal chunk".to_string(),
        ));
    }

    if plaintext_written != expected_plaintext {
        return Err(Error::InvalidFormat(format!(
            "plaintext size mismatch, expected {}, got {}",
            expected_plaintext, plaintext_written
        )));
    }

    Ok(ChunkDecryptionStats {
        chunk_count: expected_index,
        plaintext_bytes: plaintext_written,
    })
}

pub fn scan_protected_stream_structural(
    reader: &mut impl Read,
    chunk_size: u32,
) -> Result<ChunkDecryptionStats> {
    let mut expected_index = 0_u64;
    let mut plaintext_bytes = 0_u64;
    let mut saw_last = false;

    loop {
        let maybe = read_chunk_frame(reader)?;
        let Some((frame, ciphertext)) = maybe else {
            if expected_index == 0 {
                return Err(Error::InvalidFormat("missing chunk frames".to_string()));
            }
            break;
        };

        validate_protected_stream_frame(&frame, ciphertext.len(), chunk_size, expected_index)?;
        plaintext_bytes += frame.plaintext_len as u64;
        expected_index += 1;

        if frame.is_last {
            saw_last = true;
            break;
        }
    }

    if !saw_last {
        return Err(Error::InvalidFormat(
            "missing terminal chunk marker".to_string(),
        ));
    }

    if read_chunk_frame(reader)?.is_some() {
        return Err(Error::InvalidFormat(
            "unexpected trailing chunk data after terminal chunk".to_string(),
        ));
    }

    Ok(ChunkDecryptionStats {
        chunk_count: expected_index,
        plaintext_bytes,
    })
}

fn validate_protected_stream_frame(
    frame: &ChunkFrameHeader,
    ciphertext_len: usize,
    chunk_size: u32,
    expected_index: u64,
) -> Result<()> {
    validate_chunk_frame(frame, ciphertext_len, chunk_size, expected_index)?;

    if !frame.is_last && frame.plaintext_len == 0 {
        return Err(Error::InvalidFormat(
            "non-terminal chunks must carry plaintext bytes".to_string(),
        ));
    }

    Ok(())
}

fn emit_progress(
    on_progress: &mut Option<&mut dyn FnMut(ProgressStatus)>,
    started: Instant,
    phase: &str,
    current: u64,
    total: u64,
) {
    let elapsed = started.elapsed().as_secs_f64().max(0.000_001);
    let rate = if current == 0 {
        None
    } else {
        Some(current as f64 / elapsed)
    };

    let eta = match (rate, total > current) {
        (Some(rate), true) if rate > 0.0 => Some(((total - current) as f64 / rate).ceil() as u64),
        _ => None,
    };

    if let Some(handler) = on_progress.as_deref_mut() {
        handler(ProgressStatus::new(phase, current, total, eta, rate));
    }
}

fn is_cancelled(cancellation: Option<&CancellationToken>) -> bool {
    cancellation.is_some_and(|token| token.is_cancelled())
}

fn random_nonce() -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::base::Error;

    use super::*;
    use crate::domains::timelocked_file::{read_chunk_frame, write_chunk_frame, ChunkFrameHeader};

    fn test_key() -> [u8; 32] {
        [9_u8; 32]
    }

    fn test_superblock_digest() -> [u8; 32] {
        [4_u8; 32]
    }

    fn encrypt_chunk(
        key: &[u8; 32],
        superblock_digest: &[u8; 32],
        index: u64,
        is_last: bool,
        plaintext: &[u8],
        nonce: [u8; 24],
    ) -> Vec<u8> {
        let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
        cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &build_chunk_aad(
                        superblock_digest,
                        index,
                        plaintext.len() as u32,
                        is_last,
                    ),
                },
            )
            .expect("encrypt chunk")
    }

    #[test]
    fn encrypt_and_decrypt_protected_stream_round_trip_empty_plaintext() {
        let key = test_key();
        let digest = test_superblock_digest();

        let mut protected_stream = Vec::new();
        let stats = encrypt_protected_stream(
            &mut Cursor::new(Vec::<u8>::new()),
            &mut protected_stream,
            &key,
            &digest,
            16,
            0,
            None,
        )
        .expect("encrypt");

        assert_eq!(stats.chunk_count, 1);
        assert_eq!(stats.plaintext_bytes, 0);

        let mut recovered = Vec::new();
        let decrypt_stats = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut recovered,
            &key,
            &digest,
            16,
            0,
            None,
        )
        .expect("decrypt");

        assert_eq!(decrypt_stats.plaintext_bytes, 0);
        assert!(recovered.is_empty());
    }

    #[test]
    fn encrypt_and_decrypt_protected_stream_round_trip_single_chunk() {
        let key = test_key();
        let digest = test_superblock_digest();
        let plaintext = b"hello";

        let mut protected_stream = Vec::new();
        let stats = encrypt_protected_stream(
            &mut Cursor::new(plaintext),
            &mut protected_stream,
            &key,
            &digest,
            64,
            plaintext.len() as u64,
            None,
        )
        .expect("encrypt");

        assert_eq!(stats.chunk_count, 1);

        let mut recovered = Vec::new();
        decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut recovered,
            &key,
            &digest,
            64,
            plaintext.len() as u64,
            None,
        )
        .expect("decrypt");

        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn encrypt_and_decrypt_protected_stream_round_trip_multiple_chunks() {
        let key = test_key();
        let digest = test_superblock_digest();
        let plaintext = b"hello timelocked world";

        let mut protected_stream = Vec::new();
        let stats = encrypt_protected_stream(
            &mut Cursor::new(plaintext),
            &mut protected_stream,
            &key,
            &digest,
            5,
            plaintext.len() as u64,
            None,
        )
        .expect("encrypt");

        assert!(stats.chunk_count > 1);

        let mut recovered = Vec::new();
        let decrypt_stats = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut recovered,
            &key,
            &digest,
            5,
            plaintext.len() as u64,
            None,
        )
        .expect("decrypt");

        assert_eq!(decrypt_stats.plaintext_bytes, plaintext.len() as u64);
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn changing_superblock_digest_changes_chunk_aad() {
        let aad_a = build_chunk_aad(&[1_u8; 32], 0, 12, true);
        let aad_b = build_chunk_aad(&[2_u8; 32], 0, 12, true);
        assert_ne!(aad_a, aad_b);
    }

    #[test]
    fn decrypt_protected_stream_rejects_non_contiguous_chunk_indexes() {
        let key = test_key();
        let digest = test_superblock_digest();

        let mut protected_stream = Vec::new();
        write_chunk_frame(
            &mut protected_stream,
            &ChunkFrameHeader {
                index: 1,
                is_last: true,
                plaintext_len: 0,
                ciphertext_len: 16,
                nonce: [1_u8; 24],
            },
            &[0_u8; 16],
        )
        .expect("write");

        let err = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut Vec::<u8>::new(),
            &key,
            &digest,
            64,
            0,
            None,
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err.to_string().contains("non-contiguous chunk index"));
    }

    #[test]
    fn decrypt_protected_stream_rejects_invalid_is_last_usage() {
        let key = test_key();
        let digest = test_superblock_digest();
        let nonce = [7_u8; 24];
        let ciphertext = encrypt_chunk(&key, &digest, 0, false, &[], nonce);
        let mut protected_stream = Vec::new();
        write_chunk_frame(
            &mut protected_stream,
            &ChunkFrameHeader {
                index: 0,
                is_last: false,
                plaintext_len: 0,
                ciphertext_len: ciphertext.len() as u32,
                nonce,
            },
            &ciphertext,
        )
        .expect("write");

        let err = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut Vec::<u8>::new(),
            &key,
            &digest,
            64,
            0,
            None,
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err
            .to_string()
            .contains("non-terminal chunks must carry plaintext bytes"));
    }

    #[test]
    fn decrypt_protected_stream_rejects_invalid_ciphertext_length() {
        let key = test_key();
        let digest = test_superblock_digest();

        let mut protected_stream = Vec::new();
        protected_stream.extend_from_slice(&0_u64.to_le_bytes());
        protected_stream.push(1);
        protected_stream.extend_from_slice(&0_u32.to_le_bytes());
        protected_stream.extend_from_slice(&15_u32.to_le_bytes());
        protected_stream.extend_from_slice(&[0_u8; 24]);
        protected_stream.extend_from_slice(&[0_u8; 15]);

        let err = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut Vec::<u8>::new(),
            &key,
            &digest,
            64,
            0,
            None,
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err
            .to_string()
            .contains("ciphertext length must include AEAD tag"));
    }

    #[test]
    fn decrypt_protected_stream_rejects_missing_terminal_chunk() {
        let key = test_key();
        let digest = test_superblock_digest();
        let nonce = [7_u8; 24];
        let ciphertext = encrypt_chunk(&key, &digest, 0, false, b"a", nonce);
        let mut protected_stream = Vec::new();
        write_chunk_frame(
            &mut protected_stream,
            &ChunkFrameHeader {
                index: 0,
                is_last: false,
                plaintext_len: 1,
                ciphertext_len: ciphertext.len() as u32,
                nonce,
            },
            &ciphertext,
        )
        .expect("write");

        let err = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut Vec::<u8>::new(),
            &key,
            &digest,
            64,
            1,
            None,
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err.to_string().contains("missing terminal chunk marker"));
    }

    #[test]
    fn decrypt_protected_stream_rejects_trailing_chunk_after_terminal_chunk() {
        let key = test_key();
        let digest = test_superblock_digest();

        let mut protected_stream = Vec::new();
        let nonce = [2_u8; 24];
        let ciphertext = encrypt_chunk(&key, &digest, 0, true, &[], nonce);
        write_chunk_frame(
            &mut protected_stream,
            &ChunkFrameHeader {
                index: 0,
                is_last: true,
                plaintext_len: 0,
                ciphertext_len: ciphertext.len() as u32,
                nonce,
            },
            &ciphertext,
        )
        .expect("write first");
        write_chunk_frame(
            &mut protected_stream,
            &ChunkFrameHeader {
                index: 1,
                is_last: true,
                plaintext_len: 0,
                ciphertext_len: 16,
                nonce: [3_u8; 24],
            },
            &[1_u8; 16],
        )
        .expect("write second");

        let err = decrypt_protected_stream_to_writer(
            &mut Cursor::new(protected_stream),
            &mut Vec::<u8>::new(),
            &key,
            &digest,
            64,
            0,
            None,
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err.to_string().contains("unexpected trailing chunk data"));
    }

    #[test]
    fn scan_protected_stream_structural_reports_counts() {
        let key = test_key();
        let digest = test_superblock_digest();
        let plaintext = b"scan me";

        let mut protected_stream = Vec::new();
        encrypt_protected_stream(
            &mut Cursor::new(plaintext),
            &mut protected_stream,
            &key,
            &digest,
            3,
            plaintext.len() as u64,
            None,
        )
        .expect("encrypt");

        let stats =
            scan_protected_stream_structural(&mut Cursor::new(protected_stream), 3).expect("scan");
        assert_eq!(stats.plaintext_bytes, plaintext.len() as u64);
        assert!(stats.chunk_count >= 1);
    }

    #[test]
    fn encrypted_empty_stream_emits_terminal_chunk() {
        let key = test_key();
        let digest = test_superblock_digest();
        let mut protected_stream = Vec::new();

        encrypt_protected_stream(
            &mut Cursor::new(Vec::<u8>::new()),
            &mut protected_stream,
            &key,
            &digest,
            16,
            0,
            None,
        )
        .expect("encrypt");

        let (frame, ciphertext) = read_chunk_frame(&mut Cursor::new(protected_stream))
            .expect("read")
            .expect("frame");
        assert_eq!(frame.index, 0);
        assert!(frame.is_last);
        assert_eq!(frame.plaintext_len, 0);
        assert_eq!(frame.ciphertext_len as usize, ciphertext.len());
    }
}
