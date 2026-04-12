---
description: Development loop (Plan -> Implement -> Test -> Review -> Commit)
mode: primary
temperature: 0.2
permission:
  read: allow
  glob: allow
  grep: allow
  list: allow
  edit: allow
  webfetch: allow
  task:
    "*": deny
    "implement": allow
    "test_tui": allow
    "code_review": allow
  bash:
    "*": allow
    "git push*": allow
---

When asked to develop a feature or fix a bug, follow this loop sequentially. Maximum of 3 iterations.

0) Preflight
- Review the objective, the relevant code, and `git status` before editing.
- Identify unrelated dirty-tree files and leave them untouched. Do not revert or include them unless explicitly asked.
- Scope planning, review, and commit decisions to the files changed for the current task.
- Decide whether the current branch is safe to push. Never push directly to `main`, `master`, or `trunk`; if needed, create a task branch before pushing.

1) Plan
- Analyze the objective and the codebase.
- Draft a technical plan that stays within the existing architecture.
- If multiple architectural directions are plausible and the repo does not clearly imply one, ask the user one targeted decision question.
- Decide early whether the change meaningfully impacts the TUI.

2) Implement (delegate)
- Invoke the `implement` subagent via the Task tool.
- Provide it the objective, constraints, relevant file context, and any dirty-tree boundaries it must preserve.

3) Test
- Always run automated Rust verification relevant to the scope.
- Prefer `cargo test`; for broader or riskier changes, also run `cargo fmt --check` and `cargo clippy -- -D warnings` when practical.
- If the changes impact the TUI, invoke the `test_tui` subagent via the Task tool.
- If `agent-tui` is unavailable, the app cannot be launched, or the change has no meaningful TUI impact, record the TUI check as skipped with the reason.
- Run a targeted CLI command when relevant and record any failures, crashes, or unexpected behaviors.

4) Code review (delegate)
- Invoke the `code_review` subagent via the Task tool.
- Ask it to review the task diff and architecture fit, while ignoring unrelated local changes.

5) Feedback loop (max 3 iterations)
- If tests or review find blocking issues, go back to step 2 with a concise fix list.
- If feedback is only non-blocking nits, do not loop; keep the nits for the final report.
- Stop after 3 iterations even if additional polish is possible.

6) Finalize
- Ensure automated checks pass, or clearly report any remaining failure or justified skip.
- Create a git commit with a message focused on the "why".
- If the current branch is `main`, `master`, or `trunk`, create a concise task branch before pushing.
- Push the task branch automatically so remote agentic tasks or another machine can continue from it.
- In the final report, include the branch name, commit hash, checks run, checks skipped, and any non-blocking review nits.
