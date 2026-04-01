# Troubleshooting

Common problems and fixes when using buzz.

---

## Verify buzz is working

While `buzz` is running, open another terminal and run:

```powershell
powercfg /requests
```

You should see an active `SYSTEM` or `DISPLAY` request from `buzz.exe`. If it shows `None`, something is overriding the request.

---

## Screen still turns off

**Cause:** You're using `-i` (system only). This prevents sleep but allows the display to turn off.

**Fix:** Add `-s` for display:

```powershell
buzz -s -i
```

If `-s` doesn't help, a Group Policy may be overriding it. Add `-u`:

```powershell
buzz -s -u
```

---

## Lock screen still appears

**Cause:** `SetThreadExecutionState` prevents display *sleep*, not screen *lock*. Corporate environments often enforce lock separately.

**Fix:** Add `-u` to simulate user activity:

```powershell
buzz -s -u
```

If `-u` doesn't work: your IT department may enforce lock via Group Policy that cannot be overridden by any user-level tool.

---

## `-u` has no effect

**Cause:** `SendInput` only works in the active foreground desktop session.

It won't work:
- Via SSH or Remote Desktop
- As a Windows Service
- In a disconnected session

**Fix:** Run `buzz` from a local terminal instead of a remote session.

---

## Windows SmartScreen warning when running buzz.exe

**Cause:** Windows warns about any `.exe` downloaded from the internet that isn't signed with a code-signing certificate. This is normal for all small open-source tools.

**What you see:**

> "Windows protected your PC — Microsoft Defender SmartScreen prevented an unrecognized app from starting."

**Fix:**

1. Click **"More info"**
2. Click **"Run anyway"**

This is a one-time step. Windows remembers your choice and won't warn again for this file.

**To avoid this entirely:** Install via Scoop instead of downloading the `.exe` directly:

```powershell
scoop bucket add buzz https://github.com/mahajandhruv26/buzz
scoop install buzz
```

Scoop installs never trigger SmartScreen.

---

## Command not found

**Cause:** `buzz.exe` isn't in your PATH.

**Fix:**

```powershell
# Option 1: Run directly with full path
C:\path\to\buzz.exe -h

# Option 2: Add to PATH (see Installation in README)
```

Remember to **restart your terminal** after modifying environment variables.

---

## Subprocess doesn't stop on timeout

**Cause:** `buzz` uses Win32 Job Objects to kill the entire process tree (child + grandchildren) on timeout. If a process still survives, it may have broken out of the job.

**Fix:** Try running the executable directly instead of through `cmd /C`:

```powershell
# Instead of:
buzz -t 1m cmd /C "ping 127.0.0.1"

# Use:
buzz -t 1m ping 127.0.0.1
```

---

## buzz stops when I close the terminal

**This is expected.** buzz runs inside the terminal. Close the terminal, buzz stops.

**Fix — run in a minimized window:**

In cmd.exe:
```cmd
start /min C:\path\to\buzz.exe -s -t 2h
```

In PowerShell:
```powershell
Start-Process -WindowStyle Minimized "C:\path\to\buzz.exe" -ArgumentList "-s -t 2h"
```

To stop it later:
```powershell
taskkill /IM buzz.exe
```

**Tip:** Always use `-t` with a timer when running minimized, so it stops itself automatically.

---

## Build fails

```powershell
# Clean and rebuild
cargo clean && cargo build --release

# Ensure correct toolchain
rustup default stable-x86_64-pc-windows-msvc
```

---

## buzz says "process is not running" with `-w`

**Cause:** The PID you provided doesn't exist or has already exited.

**Fix:** Check the PID is correct:

```powershell
# List running processes
tasklist | findstr "process_name"

# The PID column shows the number to use
buzz -w <correct_pid>
```

---

## High CPU usage

buzz uses less than 0.1% CPU. If you see high CPU, it's not buzz — check the subprocess you're running.

buzz's main loop sleeps for 1 second between checks. The only work it does is one Windows API call per second (~1 microsecond).
