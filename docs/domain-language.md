# Timelocked — Domain language

This note defines the *words we use consistently* across docs, UI, CLI, file format, etc.

---

## Core objects

- **Original file**: The input file the user wants to protect.

- **Original message**: Plaintext message the user wants to protect.

- **Timelocked file**: A `.timelocked` file produced by Timelocked.

- **Timelocked file header**: Human-readable metadata (e.g. JSON) stored in cleartext.

- **Timelocked file payload**: Binary encrypted data (timelock material + encrypted file chunks).

- **Key (`K`)**: Random symmetric key used to encrypt the file contents. The time-lock puzzle protects defer the obtention of the key.

---

## Core concepts

- **Time-lock puzzle**: A cryptographic puzzle whose solution requires sequential work. Timelocked uses an RSW-style repeated-squaring family of puzzles.

- **Lock**: The process of creating a `.timelocked` file.

- **Unlock**: The process of running the sequential work to recover `K`, then decrypting the original file.

- **Iterations**: Number of squarings (T) to be made for unlocking a file. It is the primary difficulty parameter. More iterations means longer unlock.

- **Iteration rate**: Number of iterations per seconds (it/s) used to compute iterations.

- **Delay**: UX-friendly input (e.g. “~3 days”) that is converted into iterations.

- **Hardware profile**: User-facing term for the iteration-rate preset used to translate target delay into total iterations. MVP uses 2-3 hardcoded profiles.

- **Calibration**: A measurement step on a given machine to estimate the iteration rate (post-MVP).

- **Progress**: How much of the sequential work has been completed. Typically shown as a percentage.

- **Creator**: The user who creates a timelocked file.

- **Receiver**: The future user the *locker* intend to send the timelocked file. e.g 'anyone', 'descendants', 'spouse', 'lawyer', 'myself', ...


