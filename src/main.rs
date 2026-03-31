// buzz — A Windows caffeinate alternative
//
// Prevents the system and/or display from sleeping using the Windows
// SetThreadExecutionState API. Optionally runs a subprocess and exits
// when it finishes.
//
// Example usage:
//   buzz -s -i -t 300          Keep screen and system awake for 5 minutes
//   buzz -s unzip filename.zip Keep display awake while unzipping
//   buzz                       Keep system awake indefinitely (Ctrl+C to stop)
//   buzz -u -t 600             Simulate user activity for 10 minutes

mod args;
mod awake;
mod job;
mod process;
mod simulate;

use std::os::windows::io::AsRawHandle;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

/// Convert a process exit code to u8, clamping values > 255.
/// Windows allows 32-bit exit codes but ExitCode::from() takes u8.
fn exit_code_to_u8(code: i32) -> u8 {
    if !(0..=255).contains(&code) {
        1 // Non-representable exit codes become generic failure
    } else {
        code as u8
    }
}

fn main() -> ExitCode {
    let config = match args::parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("buzz: {e}");
            eprintln!("Try 'buzz -h' for usage information.");
            return ExitCode::from(1);
        }
    };

    if config.help {
        args::print_help();
        return ExitCode::SUCCESS;
    }

    if config.version {
        println!("buzz {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Build the execution-state flags from the user's options.
    let flags = awake::build_flags(config.keep_display, config.keep_system);
    // Show which modes are active. If no flags given, the default is system-only.
    let show_system = config.keep_system || !config.keep_display;
    println!(
        "[buzz] Awake mode engaged{}{}{}",
        if config.keep_display {
            " [display]"
        } else {
            ""
        },
        if show_system { " [system]" } else { "" },
        match config.timeout {
            Some(s) => format!(" for {} seconds", s),
            None => String::new(),
        }
    );

    // Apply the execution state.
    awake::set(flags);

    // Register Ctrl+C handler so we always restore normal sleep.
    // The handler calls process::exit() directly for instant termination,
    // since the main thread may be blocked on sleep() or try_wait().
    ctrlc::set_handler(move || {
        println!("\n[buzz] Interrupted — restoring normal sleep behavior");
        awake::clear();
        std::process::exit(130); // 128 + SIGINT, standard convention
    })
    .expect("failed to set Ctrl+C handler");

    let exit_code;

    if let Some(pid) = config.watch_pid {
        // ── Watch an existing process by PID ──
        exit_code = run_watch_pid(&config, flags, pid);
    } else if !config.command.is_empty() {
        // ── Run a subprocess while keeping awake ──
        exit_code = run_subprocess(&config, flags);
    } else {
        // ── Idle loop — wait for timeout or Ctrl+C ──
        exit_code = run_idle(&config, flags);
    }

    // Restore normal sleep behavior.
    awake::clear();
    println!("[buzz] Normal sleep behavior restored. Goodbye.");
    ExitCode::from(exit_code)
}

/// Watch an existing process by PID. Stay awake until it exits or timeout.
fn run_watch_pid(config: &args::Config, flags: u32, pid: u32) -> u8 {
    if !process::is_alive(pid) {
        eprintln!("buzz: process {pid} is not running");
        return 1;
    }

    println!("[buzz] Watching PID {pid}");
    let start = Instant::now();
    let deadline = config.timeout.map(Duration::from_secs);

    loop {
        if !process::is_alive(pid) {
            println!("[buzz] Process {pid} has exited.");
            break;
        }

        if let Some(d) = deadline {
            if start.elapsed() >= d {
                println!("[buzz] Timeout reached.");
                break;
            }
        }

        awake::set(flags);

        if config.simulate_user {
            simulate::nudge();
        }

        std::thread::sleep(Duration::from_secs(1));
    }
    0
}

/// Idle loop: keep awake until timeout expires or Ctrl+C.
/// Ctrl+C is handled by the signal handler which calls process::exit() directly.
fn run_idle(config: &args::Config, flags: u32) -> u8 {
    let start = Instant::now();
    let deadline = config.timeout.map(Duration::from_secs);

    loop {
        if let Some(d) = deadline {
            if start.elapsed() >= d {
                println!("[buzz] Timeout reached.");
                break;
            }
        }

        // Periodically re-assert the state (defensive).
        awake::set(flags);

        // Simulate user activity if requested.
        if config.simulate_user {
            simulate::nudge();
        }

        std::thread::sleep(Duration::from_secs(1));
    }
    0
}

/// Run a subprocess while keeping awake, exit when it finishes.
/// Ctrl+C is handled by the signal handler which calls process::exit() directly.
fn run_subprocess(config: &args::Config, flags: u32) -> u8 {
    let (program, args) = (&config.command[0], &config.command[1..]);
    println!("[buzz] Running: {} {}", program, args.join(" "));

    // Create a Job Object so killing the child also kills all grandchildren.
    let _job = job::JobObject::new();

    let mut child = match std::process::Command::new(program).args(args).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("buzz: failed to start '{}': {}", program, e);
            return 1;
        }
    };

    // Assign child to job object for process-tree kill on timeout/exit.
    if let Some(ref j) = _job {
        j.assign_child(&child);
    }

    let start = Instant::now();
    let deadline = config.timeout.map(Duration::from_secs);
    let child_handle = child.as_raw_handle();

    loop {
        // Wait up to 1 second for the child to exit. This is more efficient than
        // try_wait + sleep: it wakes instantly when the child exits instead of
        // waiting for the next 1-second poll.
        let wait_result = unsafe { WaitForSingleObject(child_handle, 1000) };

        if wait_result != WAIT_TIMEOUT {
            // Child has exited — collect its status.
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = exit_code_to_u8(status.code().unwrap_or(1));
                    println!("[buzz] Command exited with code {code}.");
                    return code;
                }
                Ok(None) => { /* spurious wake, continue loop */ }
                Err(e) => {
                    eprintln!("buzz: error waiting for child: {e}");
                    return 1;
                }
            }
        }

        // Timeout check.
        if let Some(d) = deadline {
            if start.elapsed() >= d {
                println!("[buzz] Timeout reached — terminating subprocess.");
                let _ = child.kill();
                let _ = child.wait();
                return 0;
            }
        }

        awake::set(flags);

        if config.simulate_user {
            simulate::nudge();
        }
    }
}
