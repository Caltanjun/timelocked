# Timelocked - Agent guide

## Project at a glance

Timelocked is a cross-platform Rust app (CLI + TUI) for creating and unlocking timed-release files (`.timelocked`).
It encrypts content with a random key and time-locks that key using sequential cryptographic work.
The project focuses on correctness, predictable UX, and a stable file format.


## Expectations

- Long term maintainability is a core priority. If you add new functionality, first check if there are shared logic that can be extracted to a separate module. Duplicate logic across mulitple files is a code smell and should be avoided. Don't be afraid to change existing code. Don't take shortcuts by just adding local logic to solve a problem.
- Always write / update / run tests when updating code.
- Use the domain language terms consistently across code, CLI/TUI copy, and docs.
- Respect architecture import rules from `docs/technical/architecture.md`:
  - `base` can import only `base`.
  - `configuration` may import `base`. But usually just contains constants or config values without business code.
  - `domains` can import `base` (and `domains` internals).
  - `usecases` can import `base` and `domains`
  - `usecases` cannot import other `usecases`.
  - `userinterfaces` can import all folders above.
- Keep `usecases` thin: they should contain as little business code as possible and focus on orchestration only.
- Put business rules in `domains`, especially verification logic in the TimelockedFile domain.
- In `base` and `domains` write one file per object/concept/service. Helps with codebase understanding and discoverability.
- When committing, use conventional commit messages (for example `fix: ...`, `chore: ...`, `ux: ...`).
