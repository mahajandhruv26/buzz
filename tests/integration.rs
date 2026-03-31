// Integration tests for the buzz binary.
//
// These tests invoke the compiled binary as a subprocess and verify its
// observable behavior: exit codes, stdout/stderr output, timeout behavior,
// and subprocess execution.

use std::process::Command;
use std::time::{Duration, Instant};

/// Helper: path to the compiled binary.
fn buzz_bin() -> String {
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()     // deps/
        .unwrap()
        .parent()     // debug/ or release/
        .unwrap()
        .to_path_buf();
    path.push("buzz.exe");
    path.to_string_lossy().to_string()
}

// ─── Help & Usage ──────────────────────────────────────────────────────────

#[test]
fn help_flag_prints_usage_and_exits_zero() {
    let output = Command::new(buzz_bin()).arg("-h").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "exit code should be 0");
    assert!(stdout.contains("USAGE"), "should print usage section");
    assert!(stdout.contains("OPTIONS"), "should print options section");
    assert!(stdout.contains("EXAMPLES"), "should print examples section");
}

#[test]
fn long_help_flag_works() {
    let output = Command::new(buzz_bin()).arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("USAGE"));
}

// ─── Error Handling ────────────────────────────────────────────────────────

#[test]
fn unknown_flag_exits_nonzero() {
    let output = Command::new(buzz_bin()).arg("-z").output().unwrap();

    assert!(!output.status.success(), "should exit with error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown"), "stderr: {stderr}");
}

#[test]
fn missing_timeout_value_exits_nonzero() {
    let output = Command::new(buzz_bin()).arg("-t").output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires"), "stderr: {stderr}");
}

#[test]
fn invalid_timeout_value_exits_nonzero() {
    let output = Command::new(buzz_bin())
        .args(["-t", "notanumber"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid"), "stderr: {stderr}");
}

// ─── Timeout Behavior ──────────────────────────────────────────────────────

#[test]
fn timeout_exits_after_specified_seconds() {
    let start = Instant::now();
    let output = Command::new(buzz_bin())
        .args(["-t", "2"])
        .output()
        .unwrap();
    let elapsed = start.elapsed();

    assert!(output.status.success(), "should exit cleanly on timeout");
    // Should take roughly 2 seconds (allow 1-4s for scheduling variance).
    assert!(
        elapsed >= Duration::from_secs(1) && elapsed <= Duration::from_secs(5),
        "elapsed: {elapsed:?}, expected ~2s"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Timeout reached"), "stdout: {stdout}");
}

#[test]
fn timeout_zero_exits_immediately() {
    let start = Instant::now();
    let output = Command::new(buzz_bin())
        .args(["-t", "0"])
        .output()
        .unwrap();
    let elapsed = start.elapsed();

    assert!(output.status.success());
    // Should exit within ~2 seconds (one loop iteration).
    assert!(
        elapsed <= Duration::from_secs(3),
        "elapsed: {elapsed:?}, expected near-instant"
    );
}

// ─── Subprocess Execution ──────────────────────────────────────────────────

#[test]
fn runs_subprocess_and_returns_its_exit_code_success() {
    // cmd /C exit 0 — exits successfully.
    let output = Command::new(buzz_bin())
        .args(["-t", "10", "cmd", "/C", "exit 0"])
        .output()
        .unwrap();

    assert!(output.status.success(), "should propagate child exit code 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Command exited"), "stdout: {stdout}");
}

#[test]
fn runs_subprocess_and_returns_its_exit_code_failure() {
    // cmd /C exit 42 — exits with code 42.
    let output = Command::new(buzz_bin())
        .args(["-t", "10", "cmd", "/C", "exit 42"])
        .output()
        .unwrap();

    let code = output.status.code().unwrap();
    assert_eq!(code, 42, "should propagate child exit code 42");
}

#[test]
fn subprocess_nonexistent_command_exits_nonzero() {
    let output = Command::new(buzz_bin())
        .args(["this_command_does_not_exist_12345"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to start"),
        "stderr: {stderr}"
    );
}

#[test]
fn subprocess_killed_on_timeout() {
    // Run a command that would take forever, but set a 2-second timeout.
    // Use ping.exe directly (not via cmd /C) so kill terminates it.
    let start = Instant::now();
    let output = Command::new(buzz_bin())
        .args(["-t", "2", "ping", "-n", "60", "127.0.0.1"])
        .output()
        .unwrap();
    let elapsed = start.elapsed();

    assert!(output.status.success(), "timeout kill should exit 0");
    assert!(
        elapsed < Duration::from_secs(10),
        "should not wait for full ping; elapsed: {elapsed:?}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Timeout reached"),
        "stdout: {stdout}"
    );
}

// ─── Flag Combinations ────────────────────────────────────────────────────

#[test]
fn display_and_system_flags_with_timeout() {
    let output = Command::new(buzz_bin())
        .args(["-s", "-i", "-t", "1"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[display]"), "stdout: {stdout}");
    assert!(stdout.contains("[system]"), "stdout: {stdout}");
}

#[test]
fn simulate_flag_accepted() {
    let output = Command::new(buzz_bin())
        .args(["-u", "-t", "2"])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn all_flags_with_command() {
    let output = Command::new(buzz_bin())
        .args(["-s", "-i", "-u", "-t", "5", "cmd", "/C", "echo hello"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[display]"));
    assert!(stdout.contains("[system]"));
    assert!(stdout.contains("Running:"));
}

// ─── Output Format ─────────────────────────────────────────────────────────

#[test]
fn awake_engaged_message_on_startup() {
    let output = Command::new(buzz_bin())
        .args(["-t", "1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[buzz] Awake mode engaged"),
        "should print engaged message; stdout: {stdout}"
    );
}

#[test]
fn restore_message_on_clean_exit() {
    let output = Command::new(buzz_bin())
        .args(["-t", "1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Normal sleep behavior restored"),
        "should print restore message; stdout: {stdout}"
    );
}

#[test]
fn timeout_displayed_in_engaged_message() {
    let output = Command::new(buzz_bin())
        .args(["-t", "42"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("for 42 seconds"),
        "should show timeout; stdout: {stdout}"
    );
}

// ─── Version ──────────────────────────────────────────────────────────────

#[test]
fn version_flag_prints_version_and_exits_zero() {
    let output = Command::new(buzz_bin()).arg("--version").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("buzz"), "should print buzz; stdout: {stdout}");
    assert!(stdout.contains("1.0.0"), "should print version; stdout: {stdout}");
}

// ─── Watch PID (-w) ──────────────────────────────────────────────────────

#[test]
fn watch_pid_nonexistent_exits_nonzero() {
    // PID 99999999 almost certainly doesn't exist.
    let output = Command::new(buzz_bin())
        .args(["-w", "99999999"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not running"),
        "stderr: {stderr}"
    );
}

#[test]
fn watch_pid_invalid_exits_nonzero() {
    let output = Command::new(buzz_bin())
        .args(["-w", "abc"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid"), "stderr: {stderr}");
}

// ─── Default mode shows [system] ─────────────────────────────────────────

#[test]
fn default_mode_shows_system_indicator() {
    let output = Command::new(buzz_bin())
        .args(["-t", "1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[system]"),
        "default mode should show [system]; stdout: {stdout}"
    );
}

// ─── Process-tree kill on timeout ────────────────────────────────────────

#[test]
fn subprocess_tree_killed_on_timeout() {
    // Launch cmd.exe which launches ping. With job objects, both should die.
    let start = Instant::now();
    let output = Command::new(buzz_bin())
        .args(["-t", "2", "cmd", "/C", "ping -n 60 127.0.0.1"])
        .output()
        .unwrap();
    let elapsed = start.elapsed();

    assert!(output.status.success(), "timeout should exit 0");
    assert!(
        elapsed < Duration::from_secs(10),
        "process tree should be killed; elapsed: {elapsed:?}"
    );
}
