# Design Decisions

> Why we chose X over Y. Each decision is recorded as an Architecture Decision Record (ADR).

---

## ADR-001: Hand-rolled parser instead of clap

**Status:** Accepted

**Context:** We need to parse 8 CLI flags. `clap` is the standard Rust argument parser.

**Decision:** Hand-written parser (~80 lines in `args.rs`).

**Rationale:**
- `clap` adds ~30 transitive dependencies and 200+ KB to the binary
- Our grammar is trivial: 6 boolean flags, 2 value flags, positional command
- Hand-written parser is easier to understand, test, and maintain at this scale
- 45 unit tests cover all edge cases — more coverage than `clap` derives would give

**Consequences:** If we add 10+ flags in the future, migrating to `clap` would be worth the tradeoff.

---

## ADR-002: Re-assert execution state every second

**Status:** Accepted

**Context:** `SetThreadExecutionState` with `ES_CONTINUOUS` should persist until explicitly cleared.

**Decision:** Call `SetThreadExecutionState` every 1 second in the main loop.

**Rationale:**
- Third-party power tools (Lenovo Vantage, Dell Power Manager) can silently reset execution state
- Corporate Group Policy enforcement scripts may clear third-party states periodically
- Windows Update preparation can reset power states
- Cost: ~1 microsecond per call. At 1 Hz, this is unmeasurable.

**Consequences:** Slightly more syscalls than necessary in the ideal case. Eliminates an entire class of silent failures.

---

## ADR-003: process::exit() in Ctrl+C handler instead of a flag

**Status:** Accepted

**Context:** When the user presses Ctrl+C, we need to restore sleep behavior and exit.

**Decision:** The handler calls `awake::clear()` then `process::exit(130)` directly.

**Alternatives considered:**
1. Set an `AtomicBool` flag, let the main loop notice and exit — adds up to 1 second of latency
2. Use a condition variable to wake the main loop — adds complexity for no benefit

**Rationale:**
- Instant termination (0ms vs up to 1000ms)
- `awake::clear()` has already run — the only cleanup that matters
- Rust destructors don't run on `exit()`, but that's fine — no destructors are needed
- The OS clears thread-bound execution state anyway as the ultimate safety net

**Consequences:** `Drop` implementations are not called. Job Object `Drop` (which kills the process tree) doesn't fire via Ctrl+C — but the child process receives the same Ctrl+C signal and dies anyway.

---

## ADR-004: VK_NONAME for input simulation

**Status:** Accepted

**Context:** The `-u` flag needs to simulate user activity. We need a keystroke that has zero side effects.

**Decision:** Use `VK_NONAME` (0xFC).

**Alternatives evaluated:**

| Key | Why rejected |
|---|---|
| `VK_SHIFT` | Changes modifier state, can interfere with applications |
| `VK_CAPITAL` | Toggles Caps Lock |
| `VK_NUMLOCK` | Toggles Num Lock |
| `VK_SCROLL` | Toggles Scroll Lock |
| `VK_F15`-`VK_F24` | Some applications bind these (media apps, macros) |
| Mouse move (0,0) | Some apps react to mouse events |
| `VK_NONAME` (0xFC) | No visible effect, no state change, no app binding, resets idle timer |

**Rationale:** `VK_NONAME` is a reserved virtual key code with no assigned function. The OS sees it as valid user input (resetting the idle timer) but no application processes it.

---

## ADR-005: Job Objects for process-tree kill

**Status:** Accepted

**Context:** When buzz times out and kills a child process (`cmd.exe`), grandchild processes survive.

**Decision:** Wrap child processes in a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.

**Alternatives considered:**
1. `taskkill /T /PID` — requires spawning another process, may need elevation
2. Manual process tree enumeration via `CreateToolhelp32Snapshot` — complex, race-prone
3. Do nothing, document it — leaves orphaned processes running without sleep prevention

**Rationale:** Job Objects are the Windows kernel's native mechanism for process-tree management. They're atomic (no race between enumeration and kill), don't require elevation, and work via RAII (`Drop` closes the handle, killing all assigned processes).

**Consequences:** If the child process creates a nested job (rare), the assignment may fail on older Windows versions. Windows 10+ supports nested jobs, so this is not an issue for our minimum supported version.

---

## ADR-006: WaitForSingleObject instead of try_wait polling

**Status:** Accepted

**Context:** We need to detect when a child process exits while also checking timeouts and re-asserting execution state.

**Decision:** Use `WaitForSingleObject(child_handle, 1000)` instead of `try_wait() + sleep(1s)`.

**Alternatives considered:**
1. `try_wait()` + `thread::sleep(1s)` — works but adds up to 1 second latency on child exit
2. `wait()` (blocking) — prevents timeout checks, Ctrl+C handling, state re-assertion, and user simulation
3. `WaitForMultipleObjects` with a timeout event — more complex, same result

**Rationale:** `WaitForSingleObject` with a 1-second timeout gives the best of both worlds: instant wake on child exit AND periodic wakeup for timeout/state checks. Same 1-second granularity for timeouts, but zero latency on child exit.

---

## ADR-007: Human-readable duration parsing

**Status:** Accepted

**Context:** Users type `-t 7200` and have to mentally calculate "that's 2 hours." Competing tools (GNU `timeout`, `sleep`) only accept seconds.

**Decision:** Accept both plain seconds (`300`) and human-readable formats (`5m`, `2h`, `1h30m`, `1h30m45s`).

**Parsing rules:**
- Try plain integer first (backwards compatible)
- Then parse character-by-character: digits accumulate, `h`/`m`/`s` multiply and add
- Case-insensitive (`2H` = `2h`)
- Trailing number after a unit is treated as seconds (`1h30` = `1h30s`)

**Rationale:** Zero cost for users who prefer seconds. Major UX improvement for everyone else. No new dependencies — 50-line parser function.

**Consequences:** Duration strings like `1h30` (without `s`) are ambiguous — could mean 1h30m or 1h30s. We chose seconds as the trailing default because it's the smaller unit and matches `sleep` behavior.

---

## ADR-008: -d as alias for -s

**Status:** Accepted

**Context:** macOS `caffeinate` uses `-d` for display. Users coming from macOS will type `buzz -d` and get "unknown option."

**Decision:** Accept both `-s` (screen) and `-d` (display) for the same function.

**Rationale:** Zero cost to implement (one extra pattern in the match). Eliminates a common first-use error for macOS users. No ambiguity — both flags do exactly the same thing.

---

## ADR-009: No Drop guard for execution state

**Status:** Accepted

**Context:** Rust's RAII pattern suggests using a `Drop` implementation to auto-clear execution state.

**Decision:** Use explicit `awake::clear()` calls instead.

**Rationale:**
- `Drop` doesn't run on `process::exit()` (called by Ctrl+C handler)
- `Drop` doesn't run on panic abort
- Explicit calls are easier to audit ("where is cleanup happening?")
- The OS provides the ultimate guarantee: thread-bound execution state is cleared on thread/process termination

**Consequences:** Every exit path must call `awake::clear()` explicitly. Currently 2 call sites: end of `main()` and the Ctrl+C handler. The OS covers any missed paths.
