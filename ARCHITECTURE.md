# Architecture

> Internal design documentation for contributors. For usage, see [README.md](README.md).

---

## Table of Contents

- [Overview](#overview)
- [Module Design](#module-design)
  - [args.rs — Argument Parser](#argsrs--argument-parser)
  - [awake.rs — Power State Manager](#awakrs--power-state-manager)
  - [job.rs — Process-Tree Manager](#jobrs--process-tree-manager)
  - [process.rs — PID Watcher](#processrs--pid-watcher)
  - [simulate.rs — User Activity Simulator](#simulaters--user-activity-simulator)
  - [main.rs — Orchestrator](#mainrs--orchestrator)
- [Control Flow](#control-flow)
- [Windows API Integration](#windows-api-integration)
  - [SetThreadExecutionState](#setthreadexecutionstate)
  - [SendInput](#sendinput)
- [Signal Handling and Cleanup](#signal-handling-and-cleanup)
- [Security Model](#security-model)
- [Test Architecture](#test-architecture)
  - [Unit Tests](#unit-tests)
  - [Integration Tests](#integration-tests)
  - [Test Matrix](#test-matrix)
- [Dependencies](#dependencies)
- [Build and CI](#build-and-ci)
- [Design Decisions](#design-decisions)
- [Code Standards](#code-standards)

---

## Overview

`buzz` is a single-binary Windows CLI tool that prevents system and/or display sleep using the Win32 `SetThreadExecutionState` API. It optionally runs a subprocess while keeping the machine awake, and can simulate user activity to defeat idle-based screen lock policies.

**Key design principles:**

1. **Correctness over features** — always restore sleep state, never leave the system in a dirty state
2. **Zero-config** — sensible defaults, no config files, no registry, no persistent state
3. **Minimal surface** — four Win32 APIs, two crate dependencies, six source files
4. **Defensive** — re-assert execution state every second, clean up on every exit path, process-tree kill via Job Objects

---

## Module Design

### `args.rs` — Argument Parser

**Responsibility:** Parse CLI arguments into a typed `Config` struct.

**Design choice:** Hand-rolled parser instead of `clap` or `structopt`. This keeps the dependency count at 2 and the binary small. The argument grammar is simple enough that a hand-written parser is clearer than a macro-generated one.

**Public API:**

```rust
pub struct Config {
    pub keep_display: bool,    // -s
    pub keep_system: bool,     // -i
    pub timeout: Option<u64>,  // -t <seconds>
    pub simulate_user: bool,   // -u
    pub help: bool,            // -h / --help
    pub command: Vec<String>,  // remaining args
}

pub fn parse() -> Result<Config, String>;
pub fn parse_from(args: &[&str]) -> Result<Config, String>;
pub fn print_help();
fn parse_duration(input: &str) -> Result<u64, String>;
```

**`parse_from` exists for testability.** It takes a `&[&str]` instead of reading `std::env::args()`, so all 39 parser tests run without spawning a subprocess.

**`parse_duration` handles human-readable time.** Accepts plain seconds (`300`), suffixed values (`5m`, `2h`, `90s`), and combined formats (`1h30m`, `1h30m45s`). Plain integers are tried first for backwards compatibility.

**Parsing rules:**
- Flags are consumed left-to-right
- `-t` consumes the next argument as its value
- The first argument that doesn't start with `-` (and isn't a `-t` value) starts the command
- Everything from that point onward is captured verbatim as the subprocess command
- Unknown flags (anything starting with `-` that isn't recognized) return an error

**Additional features:**
- `-d` is accepted as an alias for `-s` (for caffeinate users who expect `-d` for display)
- `-V` / `--version` prints version from `Cargo.toml` via `env!("CARGO_PKG_VERSION")`
- `-w <pid>` watches an existing process by PID
- `-t` accepts human-readable durations: `5m`, `2h`, `1h30m`, `90s`, or plain seconds
- `--` explicitly ends flag parsing; everything after becomes the command

**Edge cases handled:**
- `-t` with no following argument → error
- `-t abc` (non-numeric, non-duration) → error
- `-t -5` → treated as unknown flag `-5` (starts with `-`), returns error
- `-t 5x` → invalid duration unit error
- Duplicate flags → idempotent (no error, last `-t` wins)
- No arguments → valid Config with all defaults
- `--` with no following args → empty command, no error

---

### `awake.rs` — Power State Manager

**Responsibility:** Wrap `SetThreadExecutionState` with a clean Rust API.

**Public API:**

```rust
pub const ES_CONTINUOUS: u32 = 0x80000000;
pub const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
pub const ES_DISPLAY_REQUIRED: u32 = 0x00000002;

pub fn build_flags(display: bool, system: bool) -> u32;
pub fn set(flags: u32);
pub fn clear();
```

**`build_flags` logic:**

```
display=false, system=false → ES_CONTINUOUS | ES_SYSTEM_REQUIRED    (smart default)
display=true,  system=false → ES_CONTINUOUS | ES_DISPLAY_REQUIRED   (display implies system)
display=false, system=true  → ES_CONTINUOUS | ES_SYSTEM_REQUIRED
display=true,  system=true  → ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
```

When neither flag is set, we default to `ES_SYSTEM_REQUIRED` to match caffeinate's behavior (prevent idle sleep by default). When `-s` is used alone, we do NOT add `ES_SYSTEM_REQUIRED` because `ES_DISPLAY_REQUIRED` implicitly prevents system sleep (Windows cannot sleep with the display forced on).

**`set` and `clear` are thin wrappers** around the FFI call. They exist so the rest of the crate never touches `unsafe` directly.

**`clear` always passes `ES_CONTINUOUS` alone.** This is the documented way to reset — it tells Windows "keep my continuous state but remove all requirements."

---

### `job.rs` — Process-Tree Manager

**Responsibility:** Ensure that when `buzz` kills a child process (on timeout or exit), all grandchild processes are also killed.

**Public API:**

```rust
pub struct JobObject { handle: HANDLE }

impl JobObject {
    pub fn new() -> Option<Self>;
    pub fn assign_child(&self, child: &Child) -> bool;
}

impl Drop for JobObject { /* closes handle → kills all assigned processes */ }
```

**How it works:** Creates an anonymous Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When the `JobObject` is dropped (or `buzz` exits), Windows kills every process in the job — the child AND all its descendants.

**Why this matters:** Without Job Objects, `buzz -t 60 cmd /C "long_task.exe"` only kills `cmd.exe` on timeout. `long_task.exe` continues running without sleep prevention. With Job Objects, both die.

**RAII cleanup:** The `Drop` implementation calls `CloseHandle`, which triggers the kill-on-close behavior. This means process-tree cleanup happens even on panic.

---

### `process.rs` — PID Watcher

**Responsibility:** Check if an external process (by PID) is still running. Implements caffeinate's `-w` flag.

**Public API:**

```rust
pub fn is_alive(pid: u32) -> bool;
```

**How it works:** Calls `OpenProcess` with `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION`, then `WaitForSingleObject` with a 0 timeout. If the result is `WAIT_TIMEOUT`, the process is still running. The handle is closed immediately after the check.

**Edge cases:**
- PID doesn't exist → `OpenProcess` returns null → `is_alive` returns false
- Access denied → same behavior, returns false (safe default)
- PID reuse → theoretically possible but extremely unlikely in the 1-second polling window

---

### `simulate.rs` — User Activity Simulator

**Responsibility:** Periodically inject a harmless keystroke to reset the Windows idle timer.

**Public API:**

```rust
pub fn nudge();
```

**Rate limiting:** `nudge()` is called every 1 second (from the main loop), but it internally rate-limits to fire every 30-60 seconds. The interval is:

```
interval = 30 + (SystemTime::now().subsec_nanos() % 31)
```

This produces a value between 30 and 60 seconds. The jitter is per-call (not per-session), so the interval varies naturally over time.

**State:** A single `static AtomicU64` stores the epoch-second of the last nudge. This avoids any mutex and is safe across the Ctrl+C handler boundary.

**Key choice:** `VK_NONAME` (0xFC) was chosen after evaluating alternatives:

| Key | Problem |
|---|---|
| `VK_SHIFT` | Changes modifier state, can interfere with applications |
| `VK_CAPITAL` | Toggles Caps Lock |
| `VK_NUMLOCK` | Toggles Num Lock |
| `VK_SCROLL` | Toggles Scroll Lock |
| `VK_F15`-`VK_F24` | Some applications bind these |
| `VK_NONAME` (0xFC) | No visible effect, no state change, no app binding, resets idle timer |

**SendInput vs keybd_event:** We use `SendInput` (not the deprecated `keybd_event`) because `SendInput` is the modern API that works correctly with UIPI (User Interface Privilege Isolation).

---

### `main.rs` — Orchestrator

**Responsibility:** Wire the other modules together. Contains `main()`, `run_idle()`, and `run_subprocess()`.

**No business logic lives here** — it only calls into the other modules and manages control flow.

**`run_idle()`:**
1. Record start time
2. Loop every 1 second:
   - Check timeout → break if expired
   - Check `running` flag → break if Ctrl+C
   - Re-assert execution state
   - Call `simulate::nudge()` if `-u` is set
3. Return exit code 0

**`run_subprocess()`:**
1. Spawn child via `Command::new().spawn()`
2. Loop every 1 second:
   - `child.try_wait()` → if exited, capture exit code and return
   - Check timeout → if expired, kill child and return 0
   - Check `running` flag → if Ctrl+C, kill child and return 0
   - Re-assert execution state
   - Call `simulate::nudge()` if `-u` is set
3. Return child's exit code

**Why `try_wait` instead of `wait`:** `wait` blocks until the child exits, which would prevent timeout checks, Ctrl+C handling, execution state re-assertion, and user simulation. `try_wait` is non-blocking, checked every 1 second.

**Why 1-second polling:** Balances responsiveness (timeout precision, Ctrl+C latency) against CPU usage. At 1 Hz, CPU overhead is unmeasurable. Faster polling adds no user-visible benefit.

---

## Control Flow

```
main()
  │
  ├─ args::parse()                   Parse CLI → Config
  │    └─ error? → eprintln, exit(1)
  │
  ├─ config.help? → print_help(), exit(0)
  │
  ├─ awake::build_flags()            Compute ES_* combination
  ├─ awake::set(flags)               Apply initial state
  │    └─ SetThreadExecutionState(ES_CONTINUOUS | ...)
  │
  ├─ println!("[buzz] Awake mode engaged ...")
  │
  ├─ ctrlc::set_handler(|| {         Register signal handler
  │    awake::clear();
  │    exit(0);
  │  })
  │
  ├─┬─ config.command.is_empty()?
  │ │
  │ ├─ YES → run_idle(&config, &running, flags)
  │ │         └─ loop { timeout? / ctrl+c? / set(flags) / nudge() / sleep(1s) }
  │ │
  │ └─ NO  → run_subprocess(&config, &running, flags)
  │           ├─ Command::new(program).args(args).spawn()
  │           └─ loop { try_wait? / timeout? / ctrl+c? / set(flags) / nudge() / sleep(1s) }
  │
  ├─ awake::clear()                  Restore normal sleep
  │    └─ SetThreadExecutionState(ES_CONTINUOUS)
  │
  ├─ println!("[buzz] Normal sleep behavior restored.")
  └─ exit(code)
```

---

## Windows API Integration

### SetThreadExecutionState

```c
// Win32 signature
EXECUTION_STATE SetThreadExecutionState(EXECUTION_STATE esFlags);
```

| Flag | Value | Effect |
|---|---|---|
| `ES_CONTINUOUS` | `0x80000000` | Makes the state persistent until next call. Without this, the state resets on the next idle timer check (~30 seconds). |
| `ES_SYSTEM_REQUIRED` | `0x00000001` | Informs the power manager that the system is in use. Prevents automatic sleep and hibernate. |
| `ES_DISPLAY_REQUIRED` | `0x00000002` | Informs the power manager that the display is in use. Prevents automatic display power-off. Implicitly prevents system sleep. |

**buzz's usage pattern:**

1. **Set:** `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)` — persistent system-awake state
2. **Re-assert:** Same call every 1 second in the main loop. This is defensive — `ES_CONTINUOUS` should be sufficient, but third-party power management tools and Group Policy enforcement can silently reset the state. Re-asserting is cheap (one syscall, ~1 microsecond) and eliminates this failure class.
3. **Clear:** `SetThreadExecutionState(ES_CONTINUOUS)` — removes all requirement flags while maintaining the continuous call format. This is the documented reset mechanism.

**Thread-bound cleanup:** The execution state is bound to the calling thread. If the thread terminates (crash, kill, panic), Windows automatically clears the state. This is a kernel guarantee, not a library feature. This means `buzz` cannot leave the system in a "stuck awake" state even in catastrophic failure scenarios.

**Return value:** The previous execution state is returned but `buzz` ignores it. There's no meaningful action to take on failure — the call either works or the OS is in an unexpected state.

### SendInput

```c
// Win32 signature
UINT SendInput(UINT cInputs, LPINPUT pInputs, int cbSize);
```

`buzz` sends a 2-element `INPUT` array:

1. `INPUT { type: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: 0xFC, dwFlags: 0 } }` — key down
2. `INPUT { type: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: 0xFC, dwFlags: KEYEVENTF_KEYUP } }` — key up

**UIPI (User Interface Privilege Isolation):** `SendInput` is subject to UIPI — a standard-integrity process cannot inject input into a higher-integrity process. This means `buzz` running as a normal user cannot send keystrokes to an elevated (admin) window. This is by design and is not a bug.

**Session isolation:** `SendInput` only affects the calling process's desktop session. It cannot cross session boundaries (e.g., from Session 0 services to the interactive desktop).

---

## Signal Handling and Cleanup

| Exit Path | Cleanup Mechanism | Sleep Restored? |
|---|---|---|
| Timeout (idle) | `run_idle` returns → `awake::clear()` in `main` | Yes |
| Timeout (subprocess) | `child.kill()` → `run_subprocess` returns → `awake::clear()` in `main` | Yes |
| Command finishes | `run_subprocess` returns → `awake::clear()` in `main` | Yes |
| Command fails to start | `run_subprocess` returns 1 → `awake::clear()` in `main` | Yes |
| Ctrl+C | `ctrlc` handler → `awake::clear()` → `process::exit(0)` | Yes |
| Panic | Thread terminates → OS clears execution state | Yes (kernel) |
| Task Manager kill | Process terminates → OS clears execution state | Yes (kernel) |
| Power failure | System reboots | N/A |

**Why `exit(0)` in the Ctrl+C handler:** The main thread may be in a `sleep(1s)` call or waiting on `try_wait()`. Setting a flag and waiting for the loop to notice would add up to 1 second of latency. `exit(0)` after cleanup provides instant termination. The downside is that destructors don't run — but we've already called `awake::clear()`, which is the only cleanup that matters.

**No `Drop` trait implementation:** We considered implementing `Drop` on a guard struct to auto-clear execution state. We chose explicit `awake::clear()` instead because:
- `Drop` doesn't run on `process::exit()`
- `Drop` doesn't run on `Ctrl+C` (which calls `exit`)
- Explicit calls are easier to audit
- The OS provides the ultimate cleanup guarantee anyway

---

## Security Model

| Property | Implementation |
|---|---|
| **No elevation** | All APIs used (`SetThreadExecutionState`, `SendInput`) are available to standard users |
| **No shell** | `Command::new()` calls `CreateProcessW` directly — no `cmd.exe` shell interpretation, preventing injection |
| **Session isolated** | `SendInput` cannot cross session boundaries or inject into elevated processes |
| **No persistence** | No files, registry keys, scheduled tasks, or startup entries created |
| **No network** | Zero network calls — no telemetry, no update checks, no DNS lookups |
| **Minimal unsafe** | Two `unsafe` blocks, both single FFI calls to well-documented Win32 APIs |
| **Automatic cleanup** | Execution state is thread-bound — OS clears it on thread/process termination |

---

## Test Architecture

### Unit Tests

**Location:** Inline in `src/args.rs` and `src/awake.rs`, gated behind `#[cfg(test)]`.

**Run:** `cargo test --bin buzz`

#### Argument Parser Tests (39 tests in `args::tests`)

| Category | Tests | What they verify |
|---|---|---|
| Defaults | `no_args_gives_defaults` | Empty input → all false, no timeout, no command |
| Individual flags | `display_flag`, `system_flag`, `simulate_flag`, `help_short_flag`, `help_long_flag` | Each flag sets exactly one field |
| Version flag | `version_short_flag`, `version_long_flag` | `-V` and `--version` set the version field |
| Display alias | `display_alias_d_flag`, `d_and_s_are_equivalent` | `-d` works as alias for `-s` |
| Timeout parsing | `timeout_flag`, `timeout_zero_is_valid`, `timeout_large_value` | Plain integer seconds parsed correctly |
| Duration parsing | `duration_minutes`, `duration_hours`, `duration_seconds_suffix`, `duration_hours_and_minutes`, `duration_hours_minutes_seconds`, `duration_zero_minutes`, `duration_plain_seconds` | Human-readable formats: `5m`, `2h`, `1h30m`, `1h30m45s` |
| Duration errors | `duration_invalid_unit`, `duration_invalid_text` | `5x` and `abc` rejected with clear errors |
| Timeout errors | `timeout_missing_value`, `timeout_invalid_value`, `timeout_negative_value` | Missing/invalid values return descriptive errors |
| Unknown flags | `unknown_flag_errors`, `unknown_long_flag_errors` | `-z` and `--verbose` rejected |
| Flag combinations | `all_flags_combined`, `duplicate_flags_are_idempotent`, `timeout_overwritten_by_last` | Flags compose correctly |
| Command parsing | `command_capture`, `flags_then_command`, `command_with_dashes_in_args` | Non-flag args captured as command; child flags not eaten |
| `--` separator | `double_dash_separator`, `double_dash_with_no_command` | `--` ends flag parsing; handles trailing `--` with no command |
| Watch PID | `watch_pid_flag`, `watch_pid_missing_value`, `watch_pid_invalid_value`, `watch_pid_with_flags` | `-w 1234` parsed; error on missing/invalid PID; works with other flags |

#### Execution State Tests (6 tests in `awake::tests`)

| Test | What it verifies |
|---|---|
| `default_no_flags_gives_system_only` | No flags → `ES_CONTINUOUS \| ES_SYSTEM_REQUIRED` |
| `display_only` | `-s` → `ES_CONTINUOUS \| ES_DISPLAY_REQUIRED`, no `ES_SYSTEM_REQUIRED` |
| `system_only` | `-i` → `ES_CONTINUOUS \| ES_SYSTEM_REQUIRED`, no `ES_DISPLAY_REQUIRED` |
| `both_display_and_system` | `-s -i` → all three flags combined |
| `continuous_always_set` | `ES_CONTINUOUS` present in all 4 combinations of (display, system) |
| `set_and_clear_do_not_panic` | Smoke test: calling the actual Win32 API doesn't crash |

### Integration Tests

**Location:** `tests/integration.rs`

**Run:** `cargo test --test integration`

These tests invoke the compiled `buzz.exe` binary as a subprocess and verify observable behavior.

| Category | Tests | What they verify |
|---|---|---|
| Help | `help_flag_prints_usage_and_exits_zero`, `long_help_flag_works` | `-h` and `--help` print usage, exit 0 |
| Version | `version_flag_prints_version_and_exits_zero` | `--version` prints version string, exit 0 |
| Error handling | `unknown_flag_exits_nonzero`, `missing_timeout_value_exits_nonzero`, `invalid_timeout_value_exits_nonzero` | Bad input → exit 1 + correct stderr |
| Timeout | `timeout_exits_after_specified_seconds`, `timeout_zero_exits_immediately` | `-t 2` takes ~2s; `-t 0` exits instantly |
| Subprocess | `runs_subprocess_and_returns_its_exit_code_success`, `runs_subprocess_and_returns_its_exit_code_failure`, `subprocess_nonexistent_command_exits_nonzero`, `subprocess_killed_on_timeout` | Exit code 0/42 propagated; missing command errors; child killed at timeout |
| Process tree | `subprocess_tree_killed_on_timeout` | Job Objects kill cmd.exe + grandchild processes on timeout |
| Watch PID | `watch_pid_nonexistent_exits_nonzero`, `watch_pid_invalid_exits_nonzero` | Non-existent PID errors; invalid PID errors |
| Flags | `display_and_system_flags_with_timeout`, `simulate_flag_accepted`, `all_flags_with_command` | `-s -i` output shows `[display] [system]`; `-u` accepted; all flags + command works |
| Output | `awake_engaged_message_on_startup`, `restore_message_on_clean_exit`, `timeout_displayed_in_engaged_message`, `default_mode_shows_system_indicator` | Correct `[buzz]` messages; default mode shows `[system]` |

### Test Matrix

```
cargo test                        # all 67 tests
cargo test --bin buzz             # 45 unit tests
cargo test --test integration     # 22 integration tests
cargo test timeout                # all tests matching "timeout"
cargo test -- --nocapture         # show println output during tests
```

---

## Dependencies

| Crate | Version | Purpose | Why this crate |
|---|---|---|---|
| `windows-sys` | 0.59 | FFI bindings to `SetThreadExecutionState`, `SendInput` | Official Microsoft crate. Zero-cost raw bindings — no wrappers, no overhead, no runtime. Feature-gated so we only compile what we use. |
| `ctrlc` | 3.4+ | Ctrl+C handler | Handles Windows console control events (`CTRL_C_EVENT`, `CTRL_BREAK_EVENT`) correctly. Cross-platform API. Well-maintained. |

**Why not `windows` (the high-level crate)?** `windows-sys` provides raw bindings (~0 overhead). The `windows` crate adds COM support, HRESULT wrappers, and other abstractions we don't need. Binary size and compile time are smaller with `windows-sys`.

**Why not `clap` for argument parsing?** `clap` adds ~30 transitive dependencies and 200+ KB to the binary. Our grammar is 5 flags — a 60-line hand-written parser is simpler to understand, test, and maintain.

**Transitive dependency count:** ~5 (all from `windows-sys` platform target crates like `windows_x86_64_msvc`).

---

## Build and CI

### Local Build

```powershell
cargo build --release        # binary at target\release\buzz.exe
cargo test                   # run all 67 tests
cargo clippy -- -D warnings  # lint
cargo fmt --check            # format check
```

### CI Pipeline (`.github/workflows/ci.yml`)

Runs on every push to `main` and every pull request:

1. `cargo fmt --check` — formatting
2. `cargo clippy -- -D warnings` — linting
3. `cargo build --release` — build
4. `cargo test --bin buzz` — unit tests
5. `cargo test --test integration` — integration tests

### Release Pipeline (`.github/workflows/release.yml`)

Triggered by pushing a `v*` tag:

1. Build release binary
2. Run all tests
3. Zip binary as `buzz-x86_64-pc-windows-msvc.zip`
4. Create GitHub Release with auto-generated notes
5. Attach zip to release

**To release:**

```powershell
git tag v1.0.0
git push origin v1.0.0
```

---

## Design Decisions

### Why re-assert execution state every second?

The Win32 docs say `ES_CONTINUOUS` makes the state persistent. In practice, we've observed:

- Third-party power management tools (Lenovo Vantage, Dell Power Manager) can reset execution state
- Corporate Group Policy enforcement scripts run periodically and may clear third-party states
- Windows Update preparation can reset power states

Re-asserting every second costs ~1 microsecond per call and eliminates this entire class of failure.

### Why `process::exit(0)` in the Ctrl+C handler instead of a flag?

The main loop sleeps for 1 second between iterations. If we set a flag, there's up to 1 second of delay before the loop notices. `process::exit(0)` after `awake::clear()` provides instant termination. The tradeoff is that Rust destructors don't run — but the only cleanup that matters (`awake::clear()`) has already happened.

### Why poll with `try_wait` instead of blocking on `wait`?

Blocking on `wait` would prevent:
- Timeout checking
- Ctrl+C responsiveness (the flag wouldn't be checked)
- Execution state re-assertion
- User activity simulation

Polling at 1 Hz is the simplest design that satisfies all requirements.

### Why `VK_NONAME` for input simulation?

See the comparison table in the [simulate.rs section](#simulaters--user-activity-simulator). Every other virtual key code has side effects. `VK_NONAME` is the only one that resets the idle timer without any observable effect.

### Why support both implicit and `--` command separation?

The parser stops at the first non-flag token, so `buzz -s curl -O url` just works — `curl` isn't a buzz flag, so everything from `curl` onward becomes the command. This covers 99% of use cases and matches caffeinate's behavior.

We also support `--` as an explicit separator for edge cases where the command starts with a token that looks like a buzz flag:

```powershell
buzz -s -- -my-weird-command --flag arg
```

Both styles work. `--` is never required but is available when needed.

---

## Code Standards

- **`cargo clippy`** with zero warnings (enforced in CI)
- **`cargo fmt`** before every commit (enforced in CI)
- **Unit tests** for any new parser logic or flag-building logic
- **Integration tests** for any new user-facing behavior
- **Comments** only where the code isn't self-explanatory — don't comment `let x = 5; // set x to 5`
- **No new dependencies** without justification — the current count (2) is intentional
- **No `unsafe`** outside of FFI calls — and FFI calls should be wrapped in safe functions in the appropriate module
