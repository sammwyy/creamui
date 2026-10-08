# Releasing CreamUI

This checklist is for maintainers preparing a crates.io release.

1. Update the workspace version in the root `Cargo.toml` and add release notes to `CHANGELOG.md`.
2. Run `cargo fmt --all` and `cargo test --workspace`.
3. Publish in dependency order: `creamui-reactive`, `creamui-theme`, `creamui-abi`, `creamui-core`, `creamui-router`, `creamui-jsx`, `creamui-image`, `creamui-widgets`, `creamui-render`, `creamui-macros`, `creamui-dynamic`, `creamui-ffi`, then `creamui`.
4. Before each publish, run `cargo package --list -p <crate>` and `cargo publish --dry-run -p <crate>`. A dependent crate can be packaged only after the crates it depends on are indexed by crates.io.
5. Verify the published README and docs.rs page for each crate.
6. Create a Git tag matching the release version and publish the GitHub release notes.

Examples are intentionally not published.
