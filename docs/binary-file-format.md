# Timelocked binary file format (v1)

The format is designed around 3 separate concerns:

- a small plain-text notice for curious humans
- duplicated binary superblocks for recovery-critical metadata
- an encrypted payload region that can tolerate limited corruption

The file either unlocks to the exact original bytes or fails explicitly.

## Design goals

- exact recovery or explicit failure
- limited corruption recoverability
- simple authoritative metadata
- human-readable identification at the top of the file
- full binary encoding for all recovery-critical fields

## High-level layout

```text
[file_magic="TLCK"]
[notice_len:u16-le]
[notice_utf8]
[start_superblock]
[payload_region]
[end_superblock]
```

## Role of each part

| Part | Role | Required for recovery |
| --- | --- | --- |
| `file_magic` | Identifies the file as a Timelocked file. | yes |
| `notice_utf8` | Short human-readable notice. Helps a receiver understand what the file is. | no |
| `start_superblock` | First authoritative copy of recovery-critical metadata and timelock material. | yes |
| `payload_region` | Encrypted original content plus Reed-Solomon protected storage. | yes |
| `end_superblock` | Second authoritative copy of recovery-critical metadata and timelock material. | yes |
| AEAD | Keeps payload confidential and detects tampering or wrong reconstruction. | yes |
| Reed-Solomon | Repairs limited corruption in the stored encrypted payload. | yes |

## 1) File magic and notice

### File magic

`file_magic` is the fixed 4-byte ASCII signature:

```text
TLCK
```

### Notice

`notice_len` is a little-endian `u16`.

`notice_utf8` is optional plain text for humans. It is intentionally non-authoritative and non-authenticated.

Rules:

- `notice_len` MAY be `0`.
- Writers SHOULD keep the notice short.
- Writers SHOULD cap `notice_len` at 4096 bytes.
- Readers MUST NOT use the notice as the source of truth for authenticated metadata.
- Readers MUST ignore notice corruption if a valid superblock can still be found.

Example:

```text
This is a Timelocked file. Use the Timelocked app to inspect or unlock it.
```

### Notice recovery fallback

Because the notice is not recovery-critical, a damaged `notice_len` MUST NOT make the full file unrecoverable by itself.

If `notice_len` is invalid or the start superblock is not found at the expected offset, readers MAY scan forward for `sb_start_magic` within a small implementation-defined window after `file_magic`. Implementations SHOULD keep that scan window small and SHOULD validate candidate superblocks strictly before accepting them.

## 2) Superblock copies

v1 stores exactly 2 superblock copies:

- one near the beginning of the file, after the notice
- one at the end of the file

The superblock is the authoritative recovery metadata. It is binary, deterministic, and small enough to duplicate directly.

### Authority rule

Both copies MUST contain the same `superblock_body` bytes.

Readers MUST behave as follows:

- if only one copy is valid, use it
- if both copies are valid and their `superblock_body` bytes are identical, use either one
- if both copies are invalid, fail
- if both copies are valid but their `superblock_body` bytes differ, fail (contradictory informations)

There is no majority vote in v1.

### Start superblock framing

```text
[sb_start_magic="TLSB"]
[superblock_len:u32-le]
[superblock_crc32c:u32-le]
[superblock_body]
```

### End superblock framing

```text
[superblock_body]
[superblock_crc32c:u32-le]
[superblock_len:u32-le]
[sb_end_magic="TLEB"]
```

### Superblock framing rules

- `superblock_len` is the exact byte length of `superblock_body`.
- `superblock_crc32c` is computed over `superblock_body` only.
- CRC is used for fast corruption detection and copy selection.
- CRC is not an authenticity mechanism.
- Readers MUST reject unreasonable `superblock_len` values before allocating memory.
- Writers SHOULD keep the encoded `superblock_body` well below 1 MiB.

The start copy is optimized for normal parsing from the beginning of the file. The end copy is optimized for recovery when the beginning of the file is damaged.

## 3) Superblock body

`superblock_body` is the authoritative metadata for parsing, recovery, and final validation.

It excludes:

- the plain-text notice
- the superblock wrappers
- the CRC fields
- the start and end copy magic values

All integers are little-endian unless explicitly noted otherwise. Field order is fixed.

### Superblock body fields

| Field | Type | Role |
| --- | --- | --- |
| `body_version` | `u8` | Superblock body structure version. v1 value is `1`. |
| `flags` | `u16` | Reserved for future critical behavior. v1 value is `0`. |
| `payload_plaintext_bytes` | `u64` | Exact size of the original plaintext payload. |
| `protected_stream_len` | `u64` | Exact byte length of the serialized encrypted chunk stream before Reed-Solomon padding. |
| `payload_region_len` | `u64` | Exact byte length of the stored payload region. |
| `aead_chunk_size_bytes` | `u32` | Maximum plaintext bytes per AEAD chunk. |
| `aead_cipher_id` | `u8` | Payload AEAD algorithm identifier. v1 value is `1` for `XChaCha20-Poly1305`. |
| `rs_algorithm_id` | `u8` | Payload recovery algorithm identifier. v1 value is `1` for Reed-Solomon over GF(256). |
| `rs_data_shards` | `u16` | Number of data shards per Reed-Solomon stripe. |
| `rs_parity_shards` | `u16` | Number of parity shards per Reed-Solomon stripe. |
| `rs_shard_bytes` | `u32` | Exact bytes per shard in each stripe. |
| `timelock_algorithm_id` | `u8` | Timelock algorithm identifier. v1 value is `1` for `rsw-repeated-squaring-v1`. |
| `iterations` | `u64` | Number of timelock iterations. |
| `modulus_bits` | `u16` | Timelock modulus size in bits. |
| `target_seconds_present` | `u8` | `0` if absent, `1` if present. |
| `target_seconds` | `u64` | Optional UX-facing delay target in seconds. Valid only when present. |
| `created_at_unix_seconds` | `u64` | Authenticated creation time as whole UTC seconds since the Unix epoch. |
| `original_filename_len` | `u16` | UTF-8 byte length of the original filename. `0` means the original content is a message, not a file. |
| `original_filename_utf8` | bytes | Optional authenticated original filename. |
| `hardware_profile_len` | `u16` | UTF-8 byte length of the hardware profile label. `0` allowed. |
| `hardware_profile_utf8` | bytes | Optional authenticated hardware profile label. |
| `modulus_n_len` | `u32` | Big-endian byte length of timelock modulus `n`. |
| `modulus_n_bytes_be` | bytes | Timelock modulus `n = p * q`. |
| `base_a_len` | `u32` | Big-endian byte length of timelock base `a`. |
| `base_a_bytes_be` | bytes | Timelock base `a`. |
| `wrapped_key` | 32 bytes | File key `K` masked by the timelock result. |

### Field validity rules

- unknown critical `flags` bits MUST fail closed
- `payload_plaintext_bytes` MAY be `0`
- `aead_chunk_size_bytes` MUST be greater than `0`
- `rs_data_shards` MUST be greater than `0`
- `rs_parity_shards` MUST be greater than `0`
- `rs_shard_bytes` MUST be greater than `0`
- `modulus_n_len` MUST be greater than `0`
- `base_a_len` MUST be greater than `0`
- `original_filename_utf8` MUST be valid UTF-8 if present
- `hardware_profile_utf8` MUST be valid UTF-8 if present

### Notes on superblock contents

- Recovery-critical timelock material lives in the superblock, not in the payload region.
- The superblock stores only the minimum metadata needed for reliable parsing, recovery, output naming, and basic inspection.
- v1 includes `created_at` as a small authenticated inspection field.
- Rich user-facing metadata such as creator name or creator message is intentionally excluded from the authoritative superblock in v1.

## 4) AEAD binding

The payload is encrypted with chunked AEAD.

v1 uses:

- `XChaCha20-Poly1305` for payload encryption
- one random 32-byte file key `K`
- one unique 24-byte nonce per encrypted chunk

### Superblock digest

Define:

```text
superblock_digest = BLAKE3(superblock_body)
```

The digest is computed over the authoritative `superblock_body` bytes only.

### Per-chunk associated data

Each encrypted chunk uses the following associated data:

```text
"TLCK-CHUNK-AD-v1" || superblock_digest || chunk_index || plaintext_len || is_last
```

where:

- `chunk_index` is `u64-le`
- `plaintext_len` is `u32-le`
- `is_last` is `u8`

### Role of AEAD

AEAD is still required even when Reed-Solomon is present.

Reed-Solomon helps repair some stored bytes. AEAD provides:

- confidentiality: the original content stays unreadable until `K` is recovered
- integrity: modified or wrongly reconstructed ciphertext fails to decrypt
- structure binding: chunk order, chunk length, and terminal status are authenticated

If a payload byte is still wrong after Reed-Solomon recovery, AEAD MUST fail loudly.

## 5) Payload region

`payload_region` stores the encrypted payload and its Reed-Solomon protected representation.

It does not store plaintext.

It does not store the authoritative timelock metadata.

### Logical payload construction

The payload is built in 2 layers.

#### Layer A: protected stream

First, the original plaintext is split into AEAD chunks and serialized as one contiguous encrypted stream.

Each chunk frame has the following binary layout:

```text
[chunk_index:u64-le]
[is_last:u8]
[plaintext_len:u32-le]
[ciphertext_len:u32-le]
[nonce:24]
[ciphertext_with_tag]
```

Rules:

- `chunk_index` starts at `0` and MUST be contiguous
- `is_last` MUST be `1` only on the terminal chunk
- `ciphertext_len` MUST equal `plaintext_len + 16`
- `nonce` MUST be unique per chunk
- `ciphertext_with_tag` is the AEAD ciphertext plus the Poly1305 tag

Concatenating all chunk frames produces the logical `protected_stream`.

`protected_stream_len` is the exact byte length of that stream before any Reed-Solomon padding.

#### Layer B: stored payload region

Second, the `protected_stream` is split into Reed-Solomon stripes.

Each stripe contains:

- `rs_data_shards` data shards
- `rs_parity_shards` parity shards

Each shard payload is exactly `rs_shard_bytes` bytes.

Each serialized shard record is:

```text
[shard_crc32c:u32-le]
[shard_bytes]
```

The stored order inside each stripe is:

```text
[data_shard_0_record]
[data_shard_1_record]
...
[data_shard_(N-1)_record]
[parity_shard_0_record]
[parity_shard_1_record]
...
[parity_shard_(M-1)_record]
```

where:

- `N = rs_data_shards`
- `M = rs_parity_shards`

The last stripe is zero-padded as needed before parity generation.

After Reed-Solomon decoding, the reader reconstructs the original `protected_stream` and discards any trailing bytes past `protected_stream_len`.

### Role of shard CRCs

`shard_crc32c` is computed over `shard_bytes` only.

It exists to cheaply classify damaged shard records before Reed-Solomon recovery.

If a shard CRC does not match, the reader SHOULD treat that shard as an erasure input for Reed-Solomon decoding.

Like the superblock CRC, shard CRC is not an authenticity mechanism.

### Reed-Solomon scope

In v1, Reed-Solomon protects the serialized encrypted chunk stream, not plaintext and not superblock bytes.

This keeps the responsibilities clear:

- superblock copies protect small critical metadata
- Reed-Solomon protects bulk stored encrypted bytes
- AEAD verifies the final recovered bytes

## 6) Reader behavior

### Normal parse path

Readers SHOULD follow this order:

1. validate `file_magic`
2. read the notice when possible
3. read the start superblock
4. validate and decode `payload_region`
5. validate the end superblock
6. compare the 2 superblock bodies when both are valid
7. decrypt and validate the payload

### Recovery parse path

If the beginning of the file is damaged, readers MAY recover by:

1. locating the end superblock from the end of the file
2. validating its wrapper and CRC
3. using its `payload_region_len` to locate the payload region
4. decoding the payload region
5. performing normal AEAD validation

## 7) Validation and failure rules

### Superblock validation

A superblock copy is valid only if:

- its wrapper magic is correct
- its stated length is sane
- its CRC matches
- its body parses successfully
- its algorithm identifiers are known
- its numeric parameters are structurally valid

### Payload validation

A payload is valid only if:

- the payload region length matches `payload_region_len`
- Reed-Solomon reconstruction succeeds within the configured parity budget
- the reconstructed protected stream parses into a valid chunk sequence
- all AEAD chunk decryptions succeed
- the recovered plaintext byte count matches `payload_plaintext_bytes`

### Failure behavior

v1 is full-file recovery only.

Readers MUST fail if:

- both superblock copies are unusable
- both superblock copies are usable but disagree
- the payload region is too damaged for Reed-Solomon recovery
- chunk framing is malformed
- AEAD authentication fails
- the recovered plaintext size does not match the superblock

v1 does not define partial salvage semantics.

## 8) Summary of responsibilities

- plain-text notice: human hint only
- superblocks: authoritative parsing and recovery metadata
- shard CRCs: cheap corruption detection for shard selection
- Reed-Solomon: limited repair of stored encrypted payload bytes
- AEAD: confidentiality and final exact-byte integrity

## 9) Non-goals for v1

- backward compatibility with the previous draft format
- more than 2 superblock copies
- majority voting across superblock copies
- authenticated plain-text notice
- partial payload salvage after corruption beyond the parity budget
- storing rich creator-facing metadata inside the authoritative superblock

## 10) Selection policy notes

This format records the chosen AEAD and Reed-Solomon parameters per file. The file format does not require a single global chunk size or shard size for all artifacts.

In particular:

- `aead_chunk_size_bytes` and `rs_shard_bytes` are separate fields and MAY differ
- this implementation keeps `rs_data_shards = 4` and `rs_parity_shards = 2`
- this implementation chooses `rs_shard_bytes` from `protected_stream_len`: power-of-two buckets with a 64-byte floor for small payloads, and 64 KiB for larger payloads
- the policy used to choose these values at lock time is implementation behavior, not part of the binary format itself
