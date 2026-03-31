# buzz

> Keep your Windows machine awake from the command line. Like macOS `caffeinate`, but for Windows.

[![CI](https://github.com/mahajandhruv26/buzz/actions/workflows/ci.yml/badge.svg)](https://github.com/mahajandhruv26/buzz/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

<p align="center">
  <img src="demo.gif" alt="buzz demo" width="600">
</p>

Single binary. ~200 KB. No installation. No dependencies. No admin rights.

---

## Quick Start

```powershell
# Build
cargo build --release

# Run
.\target\release\buzz.exe -h
```

Or copy `buzz.exe` anywhere on your machine and run it. That's it — no installer, no setup.

**Verify it's working** — while `buzz` is running, open another terminal:

```powershell
powercfg /requests
```

You should see an active `SYSTEM` or `DISPLAY` request from `buzz.exe`.

---

## Why buzz?

| Without buzz | With buzz |
|---|---|
| Start a 2-hour build, come back to find Windows slept at 15 minutes | `buzz -i cargo build` — sleeps only after build completes |
| Present to a client, screen locks every 5 minutes | `buzz -s -u -t 2h` — display on, lock defeated, auto-stops |
| Change power settings manually, forget to change them back | `buzz -t 5m` — always restores automatically |
| Download a 50 GB file, transfer dies mid-way | `buzz -i curl -O url` — system stays awake until download finishes |
| Write a PowerShell workaround that doesn't handle crashes | `buzz` cleans up on Ctrl+C, crash, kill, or panic |

---

## Features

| Feature | Flag | Description |
|---|---|---|
| System sleep prevention | `-i` | Prevents Windows from entering sleep or hibernate |
| Display sleep prevention | `-s` / `-d` | Keeps the screen on |
| Timed awakening | `-t <dur>` | Auto-exit after duration (`5m`, `2h`, `1h30m`, or seconds) |
| Run a command | `[COMMAND]` | Stay awake while a subprocess runs, exit when it finishes |
| Watch a process | `-w <pid>` | Stay awake until an existing process exits |
| User activity simulation | `-u` | Simulates keystrokes to defeat screen lock policies |
| Graceful cleanup | — | Always restores normal sleep on exit, Ctrl+C, crash, or kill |
| Smart defaults | — | No flags = system awake indefinitely |
| Composable flags | — | All flags combine freely in any order |
| Stdout logging | — | Clear status messages at every state transition |
| Zero dependencies | — | Single .exe, ~200 KB, runs on any Windows 10+ machine |

---

## Installation

### Option 1: Build from Source

**Prerequisites:** [Rust 1.70+](https://rustup.rs/) with the `stable-x86_64-pc-windows-msvc` target.

```powershell
git clone https://github.com/mahajandhruv26/buzz.git
cd buzz
cargo build --release
```

Binary: `target\release\buzz.exe`

### Option 2: Download from Releases

Download the latest `buzz-x86_64-pc-windows-msvc.zip` from [GitHub Releases](https://github.com/mahajandhruv26/buzz/releases), extract, and run.

### Add to PATH

So you can type `buzz` from anywhere:

1. Press `Win + S` and search **"Environment Variables"**
2. Click **"Edit the system environment variables"** then **"Environment Variables"**
3. Under **User variables**, select **Path** and click **Edit**
4. Click **New** and add the folder containing `buzz.exe`
5. Restart your terminal

**Or** copy to a system directory (requires admin):

```powershell
copy target\release\buzz.exe C:\Windows\System32\
```

**Verify:**

```powershell
buzz -h
```

---

## User Guide

### Syntax

```
buzz [OPTIONS] [COMMAND ...]
```

Flags are parsed left-to-right. The first argument that doesn't start with `-` (and isn't a value for `-t`) starts the command. Everything from that point onward is passed to the subprocess.

### Flags

| Flag | Argument | What it does | Default |
|---|---|---|---|
| `-s`, `-d` | — | Keep the **display** awake (`-d` is an alias for caffeinate users) | Off |
| `-i` | — | Keep the **system** awake (prevent sleep/hibernate) | Off |
| `-t` | `<duration>` | Auto-exit after duration. Accepts: `300`, `5m`, `2h`, `1h30m`, `90s` | Indefinite |
| `-u` | — | Simulate user activity every 30-60 seconds | Off |
| `-w` | `<pid>` | Watch an existing process; exit when it terminates | — |
| `-h`, `--help` | — | Print help and exit | — |
| `-V`, `--version` | — | Print version and exit | — |
| `--` | — | Stop flag parsing; everything after is the command | — |

> **Flag naming:** `-s` = **s**creen, `-i` = **i**dle. Different from caffeinate's `-d`/`-s`. Use `-d` if you prefer caffeinate-style.

**Default (no flags):** System awake indefinitely until Ctrl+C.

**Flag order** does not matter. `-s -i -t 5m` and `-t 5m -s -i` are identical.

**`--` separator:** Use when your command starts with a dash:

```powershell
buzz -s -- -my-weird-command --flag arg
```

---

### Keep the System Awake (`-i`)

Prevents Windows from entering sleep or hibernate. CPU, disk, and network stay active. The display may still turn off.

```powershell
buzz -i                              # indefinitely
buzz -i -t 600                       # for 10 minutes
buzz -i robocopy C:\data D:\backup   # while backup runs
```

**Use when:** File transfers, database migrations, overnight batch jobs, any background task where the screen doesn't matter.

---

### Keep the Display On (`-s`)

Prevents the monitor from turning off. The system also stays awake (you can't have a screen on with a sleeping system).

```powershell
buzz -s                    # indefinitely
buzz -s -t 1800            # for 30 minutes
buzz -s npm run build      # while build runs
```

**Use when:** Presentations, dashboards, video calls, reading without touching the mouse, digital signage.

---

### Set a Timer (`-t`)

Automatically exits after the specified duration. Accepts plain seconds or human-readable formats. Works in both idle mode and with a command.

```powershell
buzz -t 300                          # 5 minutes (plain seconds)
buzz -t 5m                           # 5 minutes (human-readable)
buzz -s -t 1h                        # display awake for 1 hour
buzz -s -t 1h30m cargo build         # awake during build, max 90 min
buzz -t 2h30m45s                     # hours + minutes + seconds
```

**With a command:** Whichever happens first wins — if the command finishes before the timer, `buzz` exits immediately. If the timer expires first, `buzz` kills the command and exits.

**Use when:** Known-duration tasks, battery safety net, CI/CD hard timeouts, preventing runaway processes.

---

### Run a Command

Pass any command after the flags. `buzz` keeps the system awake while it runs and exits with the command's exit code.

```powershell
buzz -s unzip archive.zip
buzz -i python train_model.py --epochs 100
buzz -s -i -t 1800 cargo build --release
buzz -s curl -L -O https://example.com/big-file.tar.gz
buzz -i cmd /C "build.bat && deploy.bat"
```

**Exit code propagation** — use `buzz` in scripts:

```powershell
buzz -i cargo test
if ($LASTEXITCODE -ne 0) { Write-Error "Tests failed!" }
```

**How the command is parsed:** `buzz` stops flag parsing at the first non-flag argument. So `buzz -s curl -O url` passes `-O url` to curl, not to buzz.

**Use when:** Builds, downloads, ML training, database migrations, CI pipelines — any long-running command where you don't know the duration upfront.

---

### Simulate User Activity (`-u`)

Sends a harmless invisible keystroke (`VK_NONAME`) every 30-60 seconds to reset the Windows idle timer. This defeats screen lock policies that `SetThreadExecutionState` alone cannot override.

```powershell
buzz -u                    # simulate activity indefinitely
buzz -u -t 2h              # for 2 hours
buzz -u -s -i              # with display + system awake
buzz -u -s python job.py   # while a task runs
```

The interval is randomized (30-60s) so the pattern doesn't look robotic.

**Limitations:**
- Only works in the active foreground desktop session (not via SSH, RDP, or as a Service)
- Cannot defeat credential-based lock policies (smart card removal, etc.)
- Intended for legitimate use: presentations, monitoring, long tasks

**Use when:** Corporate laptops with aggressive lock policies, presentations, long meetings where you're listening but not typing.

---

### Combine Flags

All flags are composable. Mix and match freely:

| Command | Effect |
|---|---|
| `buzz` | System awake, indefinite |
| `buzz -s` | Display awake, indefinite |
| `buzz -s -i` | Display + system, indefinite |
| `buzz -t 5m` | System awake, 5 minutes |
| `buzz -s -t 5m` | Display awake, 5 minutes |
| `buzz -u` | System awake + activity simulation, indefinite |
| `buzz -s -u -t 1h` | Display + simulation, 1 hour |
| `buzz -w 1234` | System awake until PID 1234 exits |
| `buzz -s -i -u -t 10m cmd` | Everything at once |

---

### Ctrl+C and Cleanup

Press **Ctrl+C** at any time. `buzz` will:

1. Restore normal sleep behavior
2. Kill the subprocess (if one is running)
3. Exit cleanly

```
[buzz] Interrupted — restoring normal sleep behavior
```

Normal sleep is **always** restored — even if `buzz` is killed via Task Manager or crashes. The OS cleans up automatically because the execution state is thread-bound.

---

### Status Messages

`buzz` logs every state transition to stdout:

```
[buzz] Awake mode engaged [display] [system] for 300 seconds
[buzz] Running: cargo build --release
[buzz] Simulated user activity (nudge).
[buzz] Timeout reached.
[buzz] Command exited with code 0.
[buzz] Normal sleep behavior restored. Goodbye.
```

Error messages (bad flags, failed commands) go to stderr:

```
buzz: unknown option: '-z'
buzz: failed to start 'nonexistent': The system cannot find the file specified.
```

---

## Real-World Scenarios

### Overnight Build

You start a build at 6 PM and leave. Without `buzz`, Windows sleeps at 6:15 PM.

```powershell
buzz -i cargo build --release
```

System stays awake exactly as long as the build takes. If it's 10 minutes or 10 hours, doesn't matter. Sleep restores automatically when the build finishes.

### Client Presentation

Corporate policy locks your screen every 5 minutes. You're presenting to 30 people.

```powershell
buzz -s -u -t 2h
```

- `-s` keeps the display on
- `-u` defeats the lock screen timer
- `-t 2h` auto-stops after 2 hours (battery safety)

### Large Dataset Download

50 GB download that takes 3 hours. Windows sleeps after 30 minutes. The download fails.

```powershell
buzz -i -s curl -L -O https://data.example.com/dataset.tar.gz
```

System + display stay awake until curl finishes. `buzz` exits with curl's exit code.

### CI/CD Pipeline

GitHub Actions runner executing a long test suite. Need a hard timeout.

```yaml
steps:
  - name: Run tests
    run: buzz -i -t 1h cargo test
```

Prevents sleep during tests. 1-hour hard limit prevents a hung test from keeping the runner awake forever.

### Watch an Existing Process

You already started a long build. Now you want buzz to keep the system awake until it finishes.

```powershell
# Find the PID
tasklist | findstr "cargo"
# Watch it
buzz -i -w 12345
```

### Database Migration

45-minute migration. Screen doesn't matter. System must not sleep.

```powershell
buzz -i python manage.py migrate
```

### Just Keep It Awake

You don't know how long. You'll stop it when you're done.

```powershell
buzz
# ... work ...
# Ctrl+C when done
```

---

## Troubleshooting

### Verify buzz is working

While `buzz` is running, open another terminal and run:

```powershell
powercfg /requests
```

You should see an active `SYSTEM` or `DISPLAY` request from `buzz.exe`. If it shows `None`, something is overriding the request.

---

### Screen still turns off

You're using `-i` (system only). Add `-s` for display:

```powershell
buzz -s -i
```

If `-s` doesn't help, a Group Policy may be overriding it. Add `-u`:

```powershell
buzz -s -u
```

### Lock screen still appears

`SetThreadExecutionState` prevents display *sleep*, not screen *lock*. Add `-u`:

```powershell
buzz -s -u
```

If `-u` doesn't work: your IT department may enforce lock via Group Policy that cannot be overridden.

### `-u` has no effect

`SendInput` only works in the active foreground desktop session. It won't work:
- Via SSH or Remote Desktop
- As a Windows Service
- In a disconnected session

Run `buzz` from a local terminal instead.

### Command not found

`buzz.exe` isn't in your PATH. Either:

```powershell
# Run directly
C:\path\to\buzz.exe -h

# Or add to PATH (see Installation)
```

Restart your terminal after changing PATH.

### Subprocess doesn't stop on timeout

`buzz` uses Win32 Job Objects to kill the entire process tree (child + grandchildren) on timeout. If a process still survives, it may have broken out of the job. Try running the executable directly:

```powershell
# Instead of:  buzz -t 1m cmd /C "ping 127.0.0.1"
# Use:         buzz -t 1m ping 127.0.0.1
```

### Build fails

```powershell
cargo clean && cargo build --release
rustup default stable-x86_64-pc-windows-msvc
```

---

## Comparison with macOS caffeinate

| Feature | macOS `caffeinate` | Windows `buzz` |
|---|---|---|
| Prevent system sleep | `-s` | `-i` |
| Prevent display sleep | `-d` | `-s` / `-d` |
| Prevent disk sleep | `-m` | N/A |
| Timed mode | `-t <seconds>` | `-t <duration>` (`5m`, `2h`, `1h30m`, or seconds) |
| Run a command | `caffeinate cmd` | `buzz cmd` |
| Simulate user activity | N/A | `-u` |
| Attach to PID | `-w <pid>` | `-w <pid>` |
| Default (no flags) | Prevent idle sleep | Prevent system sleep |
| Exit code propagation | Yes | Yes |
| Cleanup on signal | Yes | Yes |
| Binary size | Built into macOS | ~200 KB |

**Flag naming:** `buzz` uses `-s` for **s**creen and `-i` for **i**dle because Windows doesn't have separate disk sleep, and these names are more intuitive for Windows users.

---

## Testing

67 tests across two levels. Run them with:

```powershell
cargo test               # all tests
cargo test --bin buzz    # unit tests only (45)
cargo test --test integration  # integration tests only (22)
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed test descriptions.

---

## System Requirements

| Requirement | Detail |
|---|---|
| **OS** | Windows 10+ (Home, Pro, Enterprise, Server) |
| **Architecture** | x86_64 (64-bit) |
| **Build** | Rust 1.70+ with `stable-x86_64-pc-windows-msvc` |
| **Privileges** | Standard user (no admin required) |
| **Size** | ~200 KB |

---

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Test: `cargo fmt && cargo clippy && cargo test`
4. Submit a pull request

See [ARCHITECTURE.md](ARCHITECTURE.md) for internals, module design, and code standards.

---

## License

MIT. See [LICENSE](LICENSE).
