# Timelocked — Domain language

This note defines the *words we use consistently* across docs, UI, CLI, file format, etc.

---

## Core objects

- **Original file**: The input file the user wants to protect.

- **Original message**: Plaintext message the user wants to protect.

- **Timelocked file**: A `.timelocked` file produced by Timelocked.

- **Password-protected timelocked file**: A timelocked file whose file key is protected by both the mandatory time-lock puzzle and an additional passphrase. The passphrase does not replace the time-lock puzzle.

- **Timelocked file header**: Human-readable metadata (e.g. JSON) stored in cleartext.

- **Timelocked file payload**: Binary encrypted data (timelock material + encrypted file chunks).

- **Key (`K`)**: Random symmetric key used to encrypt the file contents. The time-lock puzzle protects defer the obtention of the key.

- **Passphrase / password**: Exact UTF-8 bytes supplied by the user to add optional password protection. UI copy may use either “passphrase” or “password”; “passphrase” emphasizes that longer phrases are expected, while CLI flag names use `--password`.

---

## Core concepts

- **Time-lock puzzle**: A cryptographic puzzle whose solution requires sequential work. Timelocked uses an RSW-style repeated-squaring family of puzzles.

- **Lock**: The process of creating a `.timelocked` file.

- **Unlock**: The process of running the sequential work to recover `K`, then decrypting the original file.

- **Inspect**: The process of reading non-secret metadata from a timelocked file, including whether it is password protected, without solving the time-lock puzzle and without prompting for a passphrase.

- **Verify**: The process of structurally validating a timelocked file without unlocking the payload. Verify does not prompt for a passphrase; full payload authentication happens during unlock.

- **Password attempt**: One passphrase submission during unlock of a password-protected timelocked file. Official clients allow 3 total attempts after the sequential time-lock work finishes, so users can recover from typos without re-solving the puzzle in the same run. This limit is user-experience behavior, not cryptographic brute-force protection.

- **Iterations**: Number of squarings (T) to be made for unlocking a file. It is the primary difficulty parameter. More iterations means longer unlock.

- **Iteration rate**: Number of iterations per seconds (it/s) used to compute iterations.

- **Delay**: UX-friendly input (e.g. “~3 days”) that is converted into iterations.

- **Hardware profile**: User-facing term for the iteration-rate preset used to translate target delay into total iterations. MVP uses 2-3 hardcoded profiles.

- **Calibration**: A measurement step on a given machine to estimate the iteration rate (post-MVP).

- **Progress**: How much of the sequential work has been completed. Typically shown as a percentage.

- **Creator**: The user who creates a timelocked file.

- **Receiver**: The future user the *locker* intend to send the timelocked file. e.g 'anyone', 'descendants', 'spouse', 'lawyer', 'myself', ...

