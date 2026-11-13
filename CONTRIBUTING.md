# Contributing to CreamUI

Thanks for helping improve CreamUI.

- Keep changes focused and include tests when behavior changes.
- Run `cargo fmt --all` and `cargo test --workspace` before opening a pull request.
- On headless Linux, use `xvfb-run -a cargo test --workspace -- --test-threads=1` so window tests have a display.
- Use English for user-facing documentation, API docs, and commit messages.
- Preserve the raw-versus-themed component boundary: raw widgets own behavior and explicit tokens; themed widgets derive their defaults from `Theme`.

By contributing, you agree that your contribution may be distributed under the repository's Apache-2.0 license.
