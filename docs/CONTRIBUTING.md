# Contributing

Guide for building, testing, and contributing to buzz.

---

## Prerequisites

- [Rust 1.70+](https://rustup.rs/) with `stable-x86_64-pc-windows-msvc` target
- Git
- Windows 10 or later

---

## Build

```powershell
git clone https://github.com/mahajandhruv26/buzz.git
cd buzz
cargo build --release
```

Binary: `target\release\buzz.exe`

---

## Test

```powershell
# All tests (67 total)
cargo test

# Unit tests only (45 tests)
cargo test --bin buzz

# Integration tests only (22 tests)
cargo test --test integration

# Specific test
cargo test timeout_exits_after_specified_seconds

# Show output during tests
cargo test -- --nocapture
```

### Unit Tests (45)

Located inline in `src/args.rs` and `src/awake.rs`, gated behind `#[cfg(test)]`.

| Category | Count | What they verify |
|---|---|---|
| Defaults | 1 | Empty input → all defaults |
| Individual flags | 5 | `-s`, `-i`, `-u`, `-h`, `--help` each set one field |
| Version flag | 2 | `-V` and `--version` |
| Display alias | 2 | `-d` works as alias for `-s` |
| Timeout parsing | 3 | Plain seconds: `300`, `0`, `86400` |
| Duration parsing | 7 | `5m`, `2h`, `90s`, `1h30m`, `1h30m45s`, `0m`, plain seconds |
| Duration errors | 2 | `5x` and `abc` rejected |
| Timeout errors | 3 | Missing value, invalid value, negative value |
| Unknown flags | 2 | `-z` and `--verbose` rejected |
| Flag combinations | 3 | All combined, duplicates, overwrite |
| Command parsing | 3 | Capture, flags+command, child flags not eaten |
| `--` separator | 2 | Ends flag parsing, handles trailing `--` |
| Watch PID | 4 | Valid PID, missing value, invalid value, combined with flags |
| Execution state | 6 | Default flags, display-only, system-only, both, continuous always set, smoke test |

### Integration Tests (22)

Located in `tests/integration.rs`. Invoke `buzz.exe` as a subprocess.

| Category | Count | What they verify |
|---|---|---|
| Help | 2 | `-h` and `--help` print usage, exit 0 |
| Version | 1 | `--version` prints version, exit 0 |
| Error handling | 3 | Unknown flag, missing timeout, invalid timeout |
| Timeout | 2 | `-t 2` takes ~2s, `-t 0` exits instantly |
| Subprocess | 4 | Exit code 0/42 propagation, missing command, killed on timeout |
| Process tree | 1 | Job Objects kill cmd.exe + grandchild |
| Watch PID | 2 | Non-existent PID, invalid PID |
| Flags | 3 | `-s -i` output, `-u` accepted, all flags + command |
| Output | 4 | Engaged message, restore message, timeout in message, default `[system]` |

---

## Lint

```powershell
# Must pass with zero warnings
cargo clippy -- -D warnings

# Format check
cargo fmt --check

# Auto-format
cargo fmt
```

---

## Code Standards

- **`cargo clippy`** with zero warnings — enforced in CI
- **`cargo fmt`** before every commit — enforced in CI
- **Unit tests** for any new parser logic or flag behavior
- **Integration tests** for any new user-facing behavior
- **Comments** only where the code isn't self-explanatory
- **No new dependencies** without justification
- **No `unsafe`** outside of FFI calls — FFI calls wrapped in safe functions

---

## Project Structure

```
buzz/
├── src/
│   ├── main.rs          Orchestrator — run_idle, run_subprocess, run_watch_pid
│   ├── args.rs          CLI parser + parse_duration + help text
│   ├── awake.rs         SetThreadExecutionState wrapper
│   ├── job.rs           Win32 Job Object for process-tree kill
│   ├── process.rs       PID watcher via OpenProcess + WaitForSingleObject
│   └── simulate.rs      SendInput keystroke simulation
├── tests/
│   └── integration.rs   22 integration tests (invoke buzz.exe binary)
├── docs/
│   ├── USER_GUIDE.md    Complete usage guide
│   ├── TROUBLESHOOTING.md Common problems and fixes
│   └── CONTRIBUTING.md  This file
├── .github/
│   └── workflows/
│       ├── ci.yml       Lint + build + test on every push/PR
│       └── release.yml  Build + publish on version tag
├── ARCHITECTURE.md      System design document
├── CHANGELOG.md         Release notes
├── README.md            Landing page
├── LICENSE              MIT
├── Cargo.toml           Project config
└── Cargo.lock           Dependency lock
```

---

## CI/CD

### CI Pipeline (`.github/workflows/ci.yml`)

Runs on every push to `main` and every pull request:

1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo build --release`
4. `cargo test --bin buzz`
5. `cargo test --test integration`

### Release Pipeline (`.github/workflows/release.yml`)

Triggered by pushing a `v*` tag:

1. Build release binary
2. Run all tests
3. Zip as `buzz-x86_64-pc-windows-msvc.zip`
4. Create GitHub Release with auto-generated notes
5. Attach zip to release

**To release:**

```powershell
# Update version in Cargo.toml
# Update CHANGELOG.md
git add -A
git commit -m "Release v1.1.0"
git tag v1.1.0
git push origin main --tags
```

---

## Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make changes
4. Ensure all checks pass: `cargo fmt && cargo clippy -- -D warnings && cargo test`
5. Commit with a clear message describing the change
6. Push and open a pull request
7. Fill in the PR template with summary and test plan

**PR requirements:**
- All CI checks pass
- New features include tests
- No increase in `cargo clippy` warnings
- Code is formatted with `cargo fmt`
