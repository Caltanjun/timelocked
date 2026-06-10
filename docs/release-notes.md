# Timelocked release notes

## Optional password protection

Timelocked now supports password-protected timelocked files. When locking, users can add an optional passphrase so the payload key is protected by both the mandatory time-lock puzzle and a password-derived wrapping key.

Password protection is a mitigation for cases where future or specialized hardware completes the sequential unlock work faster than expected. It does not replace the delay: unlock still solves the time-lock puzzle first, then asks for the passphrase only if the file is password protected.

Official clients allow up to 3 total passphrase attempts after the time-lock work completes in a single unlock run. This is typo recovery only, not cryptographic brute-force protection.

Compatibility summary: unprotected writers continue to emit v1 superblock bodies, password-protected writers emit v2 bodies, and current readers parse both v1 and v2. Older readers reject v2 files as unsupported.
