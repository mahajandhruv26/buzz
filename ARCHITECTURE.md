# Architecture

> System design document for buzz. How the application is built and why.

---

## Overview

`buzz` is a single-binary Windows CLI tool that prevents system and/or display sleep using the Win32 `SetThreadExecutionState` API. It optionally runs a subprocess while keeping the machine awake, and can simulate user activity to defeat idle-based screen lock policies.

**Design principles:**

1. **Correctness over features** — always restore sleep state, never leave the system in a dirty state
2. **Zero-config** — sensible defaults, no config files, no registry, no persistent state
3. **Minimal surface** — four Win32 APIs, two crate dependencies, six source files
4. **Defensive** — re-assert execution state every second, clean up on every exit path, process-tree kill via Job Objects

---

## System Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        User Terminal                         │
│                                                             │
│  buzz -s -t 2h cargo build --release                        │
│       │    │         │                                       │
│       │    │         └──────────── subprocess command         │
│       │    └────────────────────── timeout (2 hours)          │
│       └─────────────────────────── keep display on            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                      buzz.exe                                │
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌───────────┐  ┌───────────┐  │
│  │ args.rs  │  │ awake.rs │  │  job.rs   │  │simulate.rs│  │
│  │          │  │          │  │           │  │           │  │
│  │ Parse    │  │ Set/     │  │ Job       │  │ SendInput │  │
│  │ CLI      │──▶ Clear    │  │ Object    │  │ VK_NONAME │  │
│  │ flags    │  │ sleep    │  │ tree kill │  │ every     │  │
│  │          │  │ state    │  │           │  │ 30-60s    │  │
│  └──────────┘  └────┬─────┘  └─────┬─────┘  └─────┬─────┘  │
│                     │              │              │          │
│  ┌──────────┐       │    ┌─────────┴──────┐       │          │
│  │process.rs│       │    │   main.rs      │───────┘          │
│  │          │       │    │                │                  │
│  │ Watch    │───────┼───▶│  Orchestrator  │                  │
│  │ PID      │       │    │  run_idle()    │                  │
│  │          │       │    │  run_subprocess│                  │
│  └──────────┘       │    │  run_watch_pid │                  │
│                     │    └────────────────┘                  │
└─────────────────────┼───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    Windows Kernel                            │
│                                                             │
│  SetThreadExecutionState()    SendInput()                    │
│  ES_SYSTEM_REQUIRED           VK_NONAME keystroke            │
│  ES_DISPLAY_REQUIRED          Resets idle timer              │
│                                                             │
│  CreateJobObject()            OpenProcess()                  │
│  KILL_ON_JOB_CLOSE            WaitForSingleObject()          │
│  Process-tree termination     PID monitoring                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Module Design

### `args.rs` — Argument Parser

**Responsibility:** Parse CLI arguments into a typed `Config` struct.

**Design choice:** Hand-rolled parser instead of `clap` or `structopt`. This keeps the dependency count at 2 and the binary small. The argument grammar is simple enough that a hand-written parser is clearer than a macro-generated one.

**Parsing rules:**
- Flags are consumed left-to-right
- `-t` consumes the next argument as its value (supports `300`, `5m`, `2h`, `1h30m`)
- `-w` consumes the next argument as a PID
- `-d` is accepted as an alias for `-s`
- `--` explicitly ends flag parsing
- The first non-flag argument starts the subprocess command
- Unknown flags return an error

---

### `awake.rs` — Power State Manager

**Responsibility:** Wrap `SetThreadExecutionState` with a safe Rust API.

**Flag logic:**

```
No flags         → ES_CONTINUOUS | ES_SYSTEM_REQUIRED    (smart default)
-s only          → ES_CONTINUOUS | ES_DISPLAY_REQUIRED   (display implies system)
-i only          → ES_CONTINUOUS | ES_SYSTEM_REQUIRED
-s -i            → ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
```

**Defensive re-assertion:** State is re-applied every 1 second. While `ES_CONTINUOUS` should persist, third-party power tools and Group Policy can silently reset it. Re-asserting costs ~1 microsecond and eliminates this failure class.

**Cleanup:** `clear()` passes `ES_CONTINUOUS` alone — the documented way to remove all requirements. Thread-bound cleanup by the OS kernel serves as the ultimate safety net.

---

### `job.rs` — Process-Tree Manager

**Responsibility:** Kill the entire process tree (child + grandchildren) when buzz exits or times out.

**How it works:** Creates an anonymous Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The child process is assigned to the job after spawning. When the `JobObject` is dropped, Windows kills every process in the job.

**Why this matters:** Without Job Objects, `buzz -t 60 cmd /C "long_task.exe"` only kills `cmd.exe`. `long_task.exe` survives. With Job Objects, the entire tree dies.

**RAII cleanup:** `Drop` calls `CloseHandle`, triggering kill-on-close. Works even on panic.

---

### `process.rs` — PID Watcher

**Responsibility:** Check if an external process is still running. Implements caffeinate's `-w` flag.

**How it works:** `OpenProcess` with `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION`, then `WaitForSingleObject` with 0 timeout. `WAIT_TIMEOUT` = still running. Handle is closed immediately after each check.

**Failure modes:** PID doesn't exist or access denied → returns false (safe default).

---

### `simulate.rs` — User Activity Simulator

**Responsibility:** Periodically inject a harmless keystroke to reset the Windows idle timer.

**Key choice:** `VK_NONAME` (0xFC) — no visible output, no modifier changes, no app shortcuts, but resets the idle timer. Every other key code has side effects.

**Rate limiting:** Fires every 30-60 seconds (jittered via `subsec_nanos`). Called every 1 second from the main loop but internally rate-limited via `AtomicU64` timestamp.

---

### `main.rs` — Orchestrator

**Responsibility:** Wire modules together. Three execution modes:

1. **`run_idle()`** — Loop: check timeout → re-assert state → nudge → sleep 1s
2. **`run_subprocess()`** — Spawn child in Job Object → `WaitForSingleObject(handle, 1000)` → instant exit detection → timeout check → re-assert → nudge
3. **`run_watch_pid()`** — Poll `process::is_alive(pid)` every 1s → timeout check → re-assert → nudge

No business logic lives here — only control flow.

---

## Control Flow

```
main()
  │
  ├─ args::parse()                   Parse CLI → Config
  │    └─ error? → eprintln, exit(1)
  │
  ├─ config.help? → print_help(), exit(0)
  ├─ config.version? → print version, exit(0)
  │
  ├─ awake::build_flags()            Compute ES_* combination
  ├─ awake::set(flags)               Apply initial state
  │
  ├─ ctrlc::set_handler(|| {         Register signal handler
  │    awake::clear();
  │    exit(130);
  │  })
  │
  ├─┬─ config.watch_pid?
  │ │  └─ YES → run_watch_pid()      Poll is_alive() every 1s
  │ │
  │ ├─ config.command?
  │ │  └─ YES → run_subprocess()     Spawn in Job Object, WaitForSingleObject
  │ │
  │ └─ else → run_idle()             Loop until timeout or Ctrl+C
  │
  ├─ awake::clear()                  Restore normal sleep
  └─ exit(code)
```

---

## Windows API Integration

### SetThreadExecutionState

| Flag | Value | Effect |
|---|---|---|
| `ES_CONTINUOUS` | `0x80000000` | Persistent until explicitly cleared |
| `ES_SYSTEM_REQUIRED` | `0x00000001` | Prevents sleep and hibernate |
| `ES_DISPLAY_REQUIRED` | `0x00000002` | Prevents display power-off (implies system awake) |

**Thread-bound:** If the thread/process terminates, the OS automatically clears the state. This is a kernel guarantee.

### SendInput

Sends `VK_NONAME` (0xFC) key-down + key-up. Subject to UIPI — cannot inject into elevated or cross-session processes. This is by design.

### Job Objects

`CreateJobObjectW` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When the handle closes, all assigned processes are terminated. Used for process-tree kill on timeout.

### OpenProcess + WaitForSingleObject

Used in PID watching (`-w`). `WaitForSingleObject` with 0 timeout = non-blocking check. With 1000ms timeout = efficient wait in subprocess mode (instant wake on child exit).

---

## Signal Handling and Cleanup

| Exit Path | Cleanup Mechanism | Sleep Restored? |
|---|---|---|
| Timeout (idle) | `awake::clear()` in `main` | Yes |
| Timeout (subprocess) | Job Object dropped + `awake::clear()` | Yes |
| Command finishes | Job Object dropped + `awake::clear()` | Yes |
| Command fails to start | `awake::clear()` in `main` | Yes |
| Ctrl+C | Handler → `awake::clear()` → `exit(130)` | Yes |
| Panic | OS clears thread-bound execution state | Yes (kernel) |
| Task Manager kill | OS clears thread-bound execution state | Yes (kernel) |

**No `Drop` guard:** `Drop` doesn't run on `process::exit()` or Ctrl+C. Explicit `awake::clear()` + OS kernel guarantee is more reliable.

---

## Security Model

| Property | Implementation |
|---|---|
| **No elevation** | All APIs available to standard users |
| **No shell** | `Command::new()` → `CreateProcessW` directly. No injection surface. |
| **Session isolated** | `SendInput` can't cross sessions or inject into elevated processes |
| **No persistence** | No files, registry, scheduled tasks, or startup entries |
| **No network** | Zero network calls |
| **Minimal unsafe** | Four `unsafe` blocks — all single FFI calls to documented Win32 APIs |
| **Automatic cleanup** | Thread-bound execution state + Job Object RAII |

---

## Dependencies

| Crate | Version | Purpose | Why |
|---|---|---|---|
| `windows-sys` | 0.59 | Win32 FFI bindings | Official Microsoft crate. Zero-cost. Feature-gated. |
| `ctrlc` | 3.4+ | Ctrl+C handler | Handles Windows console events correctly. |

**Why not `clap`?** Adds ~30 transitive deps + 200 KB. Our grammar is 8 flags.
**Why not `windows` crate?** Adds COM/HRESULT abstractions we don't need.
