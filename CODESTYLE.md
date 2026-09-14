# Code Style

## Comments

- No meta-comments. Do not reference the current task, PR, issue, ticket, fix,
  or caller in a comment (e.g. "used by X", "fixes #123", "added for Y flow").
  That context belongs in the commit message or PR description, not the code.
- No reasoning comments. Do not narrate what you did, tried, or considered
  while writing the code. The final code should read as if it was always this
  way.
- No comments explaining WHAT the code does. Well-named identifiers and clear
  structure should make that obvious. Only comment the WHY when it is
  genuinely non-obvious: a hidden constraint, a subtle invariant, a workaround
  for a specific external bug.
- No external references or links to documentation, issues, Stack Overflow,
  RFCs, etc. inside the code, unless strictly necessary (e.g. pointing at a
  spec required to make sense of a non-obvious algorithm or binary format).
- No commented-out code, no `// removed` markers, no changelog-style comments.

## Code quality

- Clean code: no dead code, no unused separations/dividers, no unreachable
  branches.
- No stubs, placeholders, or half-finished implementations. If a function
  exists, it must work end to end.
- No TODO/FIXME comments left in the code. If something is missing or
  deferred, do not note it inline — record it in `TODO.md` instead (see
  below).
- Every change must be tested before it is considered done.
- Code must be formatted (`cargo fmt`) before every commit.
- Code must be loggable and debuggable: use meaningful log statements at
  the right level (error/warn/info/debug/trace) for anything that helps
  diagnose failures in the field, and avoid swallowing errors silently.

## Tracking pending work

- Anything left incomplete, deferred, or known-broken must be recorded as a
  bullet point in `TODO.md` at the project root, written in English.
- Do not encode pending work as comments in source files.
