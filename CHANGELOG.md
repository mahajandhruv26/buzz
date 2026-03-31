# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-03-31

### Added

- **System sleep prevention** (`-i`) — prevents Windows from entering sleep or hibernate via `ES_SYSTEM_REQUIRED`
- **Display sleep prevention** (`-s` / `-d`) — keeps the monitor on via `ES_DISPLAY_REQUIRED`. `-d` is an alias for caffeinate users
- **Timed awakening** (`-t <duration>`) — auto-exit after a specified duration. Supports plain seconds (`300`), human-readable formats (`5m`, `2h`, `1h30m`, `1h30m45s`), and combined units
- **Subprocess execution** — run any command while keeping the system awake; exit with the command's exit code when it finishes
- **Process-tree kill** — child processes are wrapped in a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, ensuring the entire process tree is killed on timeout or exit
- **Watch existing process** (`-w <pid>`) — stay awake until an already-running process exits (caffeinate's `-w` equivalent)
- **User activity simulation** (`-u`) — sends a harmless `VK_NONAME` keystroke every 30-60 seconds to defeat idle-based screen lock policies
- **Graceful Ctrl+C handling** — always restores normal sleep behavior on interruption; exits with code 130 (128 + SIGINT)
- **`--` separator** — explicitly ends flag parsing for commands that start with dashes
- **`--version` / `-V`** — prints version from Cargo.toml
- **Smart defaults** — no flags = system awake indefinitely
- **Instant subprocess exit detection** — uses `WaitForSingleObject` instead of polling, waking immediately when the child exits
- **Defensive re-assertion** — execution state is re-applied every 1 second to guard against third-party resets
- **Status logging** — clear `[buzz]` prefixed messages at every state transition (engaged, timeout, exit, nudge)
- **67 tests** — 45 unit tests (argument parser, execution state, duration parsing) + 22 integration tests (binary behavior, exit codes, timeouts, process-tree kill, PID watching)
- **CI/CD** — GitHub Actions workflows for CI (fmt, clippy, build, test) and Release (build + publish on tag)
- **Documentation** — README.md (user guide), ARCHITECTURE.md (contributor internals), CHANGELOG.md

### Technical Details

- Language: Rust (2021 edition)
- Binary size: ~200 KB (release, standalone .exe)
- Dependencies: `windows-sys` 0.59 (Microsoft FFI bindings), `ctrlc` 3.4+ (signal handler)
- Minimum Rust version: 1.70.0
- Target: `x86_64-pc-windows-msvc`
- License: MIT
