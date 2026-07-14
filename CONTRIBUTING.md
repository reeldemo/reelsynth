# Contributing to ReelSynth

Thank you for contributing to ReelSynth (MIT). This doc covers documentation, screenshots, and release assets.

## Development setup

```bash
git clone https://github.com/reeldemo/reelsynth.git
cd reelsynth
cargo test
cargo run -p reelsynth-app --bin reelsynth-app
```

Rust ≥ 1.85 recommended. Python bindings: `maturin develop --features python`.

## Documentation structure

| File | Purpose |
|------|---------|
| [docs/README.md](docs/README.md) | Index |
| [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) | Install, first sound |
| [docs/UI.md](docs/UI.md) | UI regions + screenshots |
| [docs/WORKFLOW.md](docs/WORKFLOW.md) | DAW handoff |
| [docs/FREE_STACK.md](docs/FREE_STACK.md) | Free tools |
| [docs/SDK.md](docs/SDK.md) | Rust / Python / CLI / FFI |
| [docs/REELDEMO_INTEGRATION.md](docs/REELDEMO_INTEGRATION.md) | Commercial Studio integration |
| [AGENTS.md](AGENTS.md) | Cursor agent guidance |

When changing behavior, update the relevant doc and [CHANGELOG.md](CHANGELOG.md).

## Screenshot capture (release assets)

Screenshots are **not committed** to keep the repo lean. They ship as GitHub Release assets versioned with the app (e.g. `v0.1.0`).

### Staging directory

```bash
mkdir -p docs-images-staging
```

### Capture (macOS)

1. Build and launch the app:

   ```bash
   cargo run -p reelsynth-app --bin reelsynth-app
   ```

2. Load a representative preset (factory Saw Morph is fine).

3. Capture the window (requires Screen Recording permission):

   ```bash
   # Find window ID (example — adjust for your WM)
   WIN_ID=$(osascript -e 'tell app "System Events" to id of window 1 of process "reelsynth-app"')
   screencapture -o -l"$WIN_ID" docs-images-staging/01-full-window.png
   ```

4. Capture detail shots (crop or re-frame):

   | File | Content |
   |------|---------|
   | `01-full-window.png` | Full 1280×880 window |
   | `02-header-midi-save.png` | Header: Open, Save, WT, MIDI, Piano |
   | `03-osc-filter-adsr.png` | Left rail + center filter/ADSR |
   | `04-wt-editor-2d-3d.png` | Wavetable strip + 2D/3D views |
   | `05-mod-fx.png` | Mod matrix + FX rack |
   | `06-piano-keyboard.png` | Piano visible |

   Optional: annotate with numbered callouts before upload.

5. Bundle:

   ```bash
   chmod +x scripts/bundle-docs-images.sh
   ./scripts/bundle-docs-images.sh
   ```

6. Create a GitHub Release tagged to `Cargo.toml` version and upload `docs-images.zip` or individual PNGs.

### URL pattern in docs

```markdown
![Alt](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png)
```

Update version in all doc links when releasing a new app version.

## Code conventions

See [docs/NAMING.md](docs/NAMING.md). Match existing module style; minimal scope in PRs.

## Tests

```bash
cargo test
cargo test --no-default-features -j 1   # UI tests without default features
```

## Binary releases

Tagged pushes (`v*`) trigger [.github/workflows/release.yml](.github/workflows/release.yml), which builds and uploads:

| Platform | Artifact |
|----------|----------|
| macOS Apple Silicon | `reelsynth-<ver>-macos-aarch64.zip` |
| macOS Intel | `reelsynth-<ver>-macos-x86_64.zip` |
| Linux x86_64 | `reelsynth-<ver>-linux-x86_64.tar.gz` |
| Windows x86_64 | `reelsynth-<ver>-windows-x86_64.zip` |

Each archive contains `bin/reelsynth-app`, `bin/reelsynth-export`, and `RELEASE_NOTES.md`.

### Local staging (macOS/Linux)

```bash
./scripts/release.sh stage    # build + zip in dist/
./scripts/release.sh info     # version, paths
./scripts/release.sh publish  # gh release create (single local artifact)
```

Bump `version` in workspace `Cargo.toml` files before tagging. Tag and push:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Manual cross-build (example — Intel macOS from Apple Silicon):

```bash
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin -p reelsynth-app -p reelsynth --bin reelsynth-export
```

## Pull requests

- Conventional commits: `feat:`, `fix:`, `docs:`, `chore:`
- Link issues with `#123` if applicable
- For UI changes: note whether screenshots need re-capture on release

## License

Contributions are MIT — same as the project.
