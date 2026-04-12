# Timelocked - Architecture (MVP)

Timelocked creates and unlocks timelocked files (`.timelocked`): the app encrypts payload bytes with a random file key `K`, then time-locks only `K` using sequential work (RSW-style repeated squaring family).

## Goals

- Correctness first: unlocking must always recover the exact original bytes.
- Predictable UX: clean CLI/TUI flows, progress + ETA, safe defaults.
- Portable: Windows/macOS/Linux builds and a stable versioned file format.
- Testable: deterministic vectors and strong invariants around file format and crypto boundaries.

## MVP constraints

- Single Cargo package for now (no workspace split).
- In-process architecture only.
- CLI + TUI only for user interfaces in MVP.
- Keep feature-first grouping in `usecases` and `userinterfaces`.
- Keep abstractions lightweight; no mandatory DI framework.

## Source code organization

Root folders are intentionally kept simple and stable:

```
/src
  /base
  /configuration
  /domains
    /timelock
    /timelocked_file
  /usecases
    /lock
    /unlock
    /inspect
    /verify
  /userinterfaces
    /common
    /cli
    /tui
  lib.rs
  main.rs
```

### Root folder responsibilities

- `base`: reusable technical primitives with no Timelocked business concepts (e.g Event, Entities).
- `configuration`: global configuration files (e.g. hardware profile list)
- `domains`: domain language and business rules (timelock, TimelockedFile concepts, verification rules, domain errors).
- `usecases`: thin application orchestration (`lock`, `unlock`, `inspect`, `verify`) with minimal business logic.
- `userinterfaces`: CLI and TUI delivery code.

## Import rules

Use these rules in code review to keep boundaries clear:

- `base` can import only `base`.
- `domains/[subdomain]` can import `base` (and same subdomain internals).
- `domains/[subdomain]` cannot import from other subdomains.
- `usecases` can import `base` and `domains`
- `usecases` cannot import other `usecases`.
- `userinterfaces` can import all folders above.

Additional rules:
- Keep UI free of business logic; UI triggers `usecases` and renders results.
- Keep `usecases` thin: orchestrate flow, IO boundaries, and progress wiring only.
- Put business rules in domains; for verification, prefer domain-level logic in `domains/timelocked_file` with `usecases/verify` as an orchestrator.


## Coordination model (MVP)

- Default to direct function calls between layers.
- Use explicit progress reporters/callbacks for long-running flows (`lock`, `unlock`).
- In-process events are optional and should be introduced only when one signal has multiple independent consumers.

## Key technical decisions

- Big integer backend and helpers for repeated squaring and parameter calculations: `num-bigint`, `num-integer`, and `num-traits`.
- Prime generation for timelock modulus construction: `glass_pumpkin`.
- Binary container format v1 is the current notice + duplicated superblocks + protected payload-region design documented in `docs/technical/binary-file-format.md`.
- Payload encryption uses chunked AEAD with `XChaCha20-Poly1305` over a serialized protected stream.
- Chunk authenticity is bound to authoritative metadata via `BLAKE3(superblock_body)` in per-chunk associated data.
- Corruption handling is layered: `CRC32C` for cheap superblock/shard damage detection, Reed-Solomon over GF(256) for limited payload-region repair, and AEAD for final exact-byte verification.
- Current payload recovery policy keeps Reed-Solomon redundancy at `4 data + 2 parity`, with adaptive shard sizing for smaller protected streams and `64 KiB` shards for larger ones.
- Authentication boundary: the superblock is authoritative and authenticated through AEAD binding; the plain-text notice is intentionally non-authoritative and non-authenticated.
- Delay estimation in MVP remains 2-3 hardcoded hardware profiles plus session-only current-machine calibration.
- Creation time is stored as authenticated Unix seconds and formatted for UI/inspect output.
- Deterministic test fixtures remain important for format and crypto invariants.
- Sensitive-memory hygiene for key material and recovered secrets uses `zeroize`.

## Testing strategy

### Unit tests

- Domain rules: parameter validation, progress math, error mapping.
- Usecases orchestration/wiring (business logic should stay in domains)
- File format parse/write invariants (round-trips, version handling).

### Integration tests

- `lock -> inspect -> unlock` flow for small fixtures.
- Corruption tests: flip bits in notice, superblocks, and payload-region bytes and assert expected failure modes.

### Smoke tests (fast, CI)

- Run binaries with `--help` and `--version`.
- Lock/unlock with tiny iteration counts to validate end-to-end wiring.

### E2E tests (slower)

- Larger fixtures plus chunked IO.
- Cross-platform differences (paths, permissions, line endings).

### Optional property tests

- Use `proptest` for parser robustness and no-panics guarantees.

## Cross-build and release

- CI builds for Linux, macOS, and Windows.
- Prefer self-contained binaries.
- Lock the Rust toolchain via `rust-toolchain.toml`.

## UI layer

- CLI: `clap` for arguments, `tracing` for logs, and `indicatif` for progress.
- TUI: `ratatui` + `crossterm` with a small UI state model.
- No business logic in UI modules.

## Dependency baseline (MVP)

Dependencies decided so far (not exhaustive):

- `num-bigint`, `num-integer`, and `num-traits` for timelock arithmetic and integer helpers.
- `glass_pumpkin` for probable-prime generation during modulus construction.
- `chacha20poly1305` (using `XChaCha20Poly1305`) for chunked AEAD payload encryption.
- `rand` for cryptographically secure file-key and nonce generation.
- `blake3` for superblock digests bound into chunk associated data.
- `crc32c` for superblock and shard corruption detection.
- `reed-solomon-erasure` for GF(256) payload-region redundancy and recovery.
- `chrono` for formatting authenticated creation timestamps in inspect/UI output.
- `serde` and `serde_json` for structured data models and CLI JSON output.
- `clap` for CLI argument parsing and `indicatif` for CLI progress rendering.
- `ratatui` + `crossterm` for TUI rendering, keyboard input, and terminal control.
- `thiserror` for domain/usecase error types; `anyhow` for binary entrypoint error plumbing.
- `tracing` and `tracing-subscriber` to configure runtime logging.
- `tempfile` for safe temp-output flows (lock/unlock writing and atomic replace patterns).
- `zeroize` for wiping key material and other sensitive buffers after use.
