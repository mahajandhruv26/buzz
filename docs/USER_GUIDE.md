# User Guide

Complete usage guide for buzz — all flags, modes, scenarios, and examples.

---

## Syntax

```
buzz [OPTIONS] [COMMAND ...]
```

Flags are parsed left-to-right. The first argument that doesn't start with `-` (and isn't a value for `-t`) starts the command. Everything from that point onward is passed to the subprocess.

---

## Flags

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

## Keep the System Awake (`-i`)

Prevents Windows from entering sleep or hibernate. CPU, disk, and network stay active. The display may still turn off.

```powershell
buzz -i                              # indefinitely
buzz -i -t 600                       # for 10 minutes
buzz -i robocopy C:\data D:\backup   # while backup runs
```

**Use when:** File transfers, database migrations, overnight batch jobs, any background task where the screen doesn't matter.

---

## Keep the Display On (`-s`)

Prevents the monitor from turning off. The system also stays awake (you can't have a screen on with a sleeping system).

```powershell
buzz -s                    # indefinitely
buzz -s -t 1800            # for 30 minutes
buzz -s npm run build      # while build runs
```

**Use when:** Presentations, dashboards, video calls, reading without touching the mouse, digital signage.

---

## Set a Timer (`-t`)

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

## Run a Command

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

## Watch an Existing Process (`-w`)

Stay awake until a process you've already started finishes.

```powershell
# Find the PID in Task Manager (Details tab) or:
tasklist | findstr "cargo"

# Watch it
buzz -i -w 12345
```

**Use when:** You already started a long task and realize you need sleep prevention. No need to restart it.

---

## Simulate User Activity (`-u`)

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

## Combine Flags

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

## Ctrl+C and Cleanup

Press **Ctrl+C** at any time. `buzz` will:

1. Restore normal sleep behavior
2. Kill the subprocess (if one is running)
3. Exit cleanly

```
[buzz] Interrupted — restoring normal sleep behavior
```

Normal sleep is **always** restored — even if `buzz` is killed via Task Manager or crashes. The OS cleans up automatically because the execution state is thread-bound.

---

## Status Messages

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
