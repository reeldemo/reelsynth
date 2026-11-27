# Continuous integration

Workflows under `.github/workflows/`:

| Workflow | Role |
|----------|------|
| `ci.yml` | Tests, Clippy code-smell gate, rustfmt (advisory), `cargo audit` + `cargo deny` |
| `codeql.yml` | GitHub CodeQL security analysis (Rust) |
| `dependency-review.yml` | PR dependency diff review (fail on high severity) |
| `release.yml` | Tagged `v*` multi-platform release builds |

Dependabot (`.github/dependabot.yml`) opens weekly PRs for Cargo crates and GitHub Actions (monthly for pip/maturin metadata).

## Local equivalents

```bash
# Tests (same as CI test job)
cargo test -p reelsynth
cargo test -p reelsynth-ui --test kittest
cargo test -p reelsynth-ui

# Code smell (correctness / suspicious hard-fail)
cargo clippy --workspace --all-targets -- \
  -D clippy::correctness -D clippy::suspicious \
  -A clippy::too_many_arguments -A clippy::needless_range_loop \
  -A clippy::manual_clamp -A clippy::collapsible_if \
  -A clippy::redundant_closure -A clippy::ptr_arg \
  -A clippy::module_inception -A clippy::arc_with_non_send_sync

# Security
cargo audit
cargo deny --workspace --all-features check advisories licenses sources
```

## Notes

- **rustfmt** is advisory until a dedicated `cargo fmt --all` PR lands (large existing drift).
- **Ignored advisories** (egui-pinned `quick-xml`, optional `pyo3 ^0.22`) are listed in `deny.toml` and `.cargo/audit.toml` — revisit when those stacks bump.
- **webbrowser** was bumped to `1.2.2` to clear RUSTSEC-2026-0257.
- **Dependency Review** skips until Dependency graph is enabled under repo Settings → Code security.
- **CodeQL** uses `build-mode: none` (Rust does not support manual builds).

