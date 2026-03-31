// Argument parsing for buzz.
//
// We roll a small hand-written parser so there are zero extra dependencies
// beyond what Cargo.toml already lists.

/// Parsed configuration from command-line arguments.
#[derive(Debug, PartialEq)]
pub struct Config {
    /// Keep display awake (-s).
    pub keep_display: bool,
    /// Keep system awake (-i).
    pub keep_system: bool,
    /// Optional timeout in seconds (-t).
    pub timeout: Option<u64>,
    /// Simulate user activity (-u).
    pub simulate_user: bool,
    /// Show help and exit.
    pub help: bool,
    /// Show version and exit.
    pub version: bool,
    /// Watch an existing process by PID (-w).
    pub watch_pid: Option<u32>,
    /// Remaining arguments form the subprocess command to run.
    pub command: Vec<String>,
}

/// Parse a duration string into seconds.
///
/// Supports:
///   - Plain seconds: "300", "60"
///   - Human-readable: "5m", "2h", "1h30m", "90s", "1h30m45s"
///   - Mixed: "2h30m" = 9000 seconds
fn parse_duration(input: &str) -> Result<u64, String> {
    // Try plain integer first (backwards compatible).
    if let Ok(secs) = input.parse::<u64>() {
        return Ok(secs);
    }

    let mut total: u64 = 0;
    let mut current_num = String::new();
    let mut found_unit = false;

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else {
            if current_num.is_empty() {
                return Err(format!("invalid duration: '{input}' (unexpected '{ch}')"));
            }
            let n: u64 = current_num
                .parse()
                .map_err(|_| format!("invalid duration: '{input}'"))?;
            current_num.clear();
            found_unit = true;
            match ch {
                'h' | 'H' => total += n * 3600,
                'm' | 'M' => total += n * 60,
                's' | 'S' => total += n,
                _ => {
                    return Err(format!(
                        "invalid duration unit '{ch}' in '{input}' (use h, m, or s)"
                    ))
                }
            }
        }
    }

    // Trailing number without unit is treated as seconds.
    if !current_num.is_empty() {
        if found_unit {
            // e.g. "1h30" — trailing number after units, treat as seconds
            let n: u64 = current_num
                .parse()
                .map_err(|_| format!("invalid duration: '{input}'"))?;
            total += n;
        } else {
            // No units found at all, but it wasn't a plain integer (handled above).
            return Err(format!("invalid duration: '{input}'"));
        }
    }

    if !found_unit && total == 0 {
        return Err(format!("invalid duration: '{input}'"));
    }

    Ok(total)
}

/// Parse a slice of string arguments into a `Config`.
/// Separated from `parse()` so it can be unit-tested without relying on
/// `std::env::args()`.
pub fn parse_from(args: &[&str]) -> Result<Config, String> {
    let mut cfg = Config {
        keep_display: false,
        keep_system: false,
        timeout: None,
        simulate_user: false,
        help: false,
        version: false,
        watch_pid: None,
        command: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        match arg {
            "-h" | "--help" => cfg.help = true,
            "-V" | "--version" => cfg.version = true,
            "-s" | "-d" => cfg.keep_display = true,
            "-i" => cfg.keep_system = true,
            "-u" => cfg.simulate_user = true,
            "-w" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| "-w requires a <pid> argument".to_string())?;
                cfg.watch_pid = Some(
                    val.parse::<u32>()
                        .map_err(|_| format!("invalid PID value: '{val}'"))?,
                );
            }
            "-t" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| "-t requires a <duration> argument".to_string())?;
                cfg.timeout = Some(parse_duration(val)?);
            }
            // "--" explicitly ends flag parsing; everything after is the command.
            "--" => {
                cfg.command = args[i + 1..].iter().map(|s| s.to_string()).collect();
                break;
            }
            // First non-flag argument starts the command.
            other if other.starts_with('-') => {
                return Err(format!("unknown option: '{other}'"));
            }
            _ => {
                // Everything from here onward is the subprocess command.
                cfg.command = args[i..].iter().map(|s| s.to_string()).collect();
                break;
            }
        }
        i += 1;
    }

    Ok(cfg)
}

/// Parse `std::env::args()` into a `Config`.
pub fn parse() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    parse_from(&refs)
}

/// Print a detailed help message.
pub fn print_help() {
    println!(
        r#"buzz v{version} — Keep your Windows machine awake (like macOS caffeinate)

USAGE
    buzz [OPTIONS] [COMMAND ...]

OPTIONS
    -s, -d        Keep the display awake (ES_DISPLAY_REQUIRED)
                  (-d is an alias for -s, for caffeinate users)
    -i            Keep the system awake (ES_SYSTEM_REQUIRED)
    -t <duration> Stay awake for <duration>, then exit
                  Accepts: 300, 5m, 2h, 1h30m, 90s
    -u            Simulate user activity every 30-60 s to prevent idle
    -w <pid>      Watch an existing process; exit when it terminates
    -h, --help    Show this help message
    -V, --version Show version and exit

    Flags: -s = screen, -i = idle (not caffeinate's -d/-s)

If no flags are given, buzz keeps the system awake indefinitely until
you press Ctrl+C.

COMMAND
    Any arguments after the flags are treated as a command to run.
    buzz keeps the machine awake while the command executes and exits
    when the command finishes (or when the timeout is reached).

    Use -- to separate buzz flags from commands that start with -:
      buzz -s -- -my-command arg1 arg2

VERIFY
    To confirm buzz is working, run in another terminal:
      powercfg /requests

EXAMPLES
    buzz                             System awake until Ctrl+C
    buzz -s -i -t 5m                 Screen + system awake for 5 min
    buzz -s unzip archive.zip        Display awake while unzipping
    buzz -u -t 2h                    Simulate activity for 2 hours
    buzz -s -t 1h30m cargo build     Display awake, run cargo build,
                                     exit when build finishes or 90 min
    buzz -w 1234                     Stay awake until PID 1234 exits
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}

// ─── Unit Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_gives_defaults() {
        let cfg = parse_from(&[]).unwrap();
        assert!(!cfg.keep_display);
        assert!(!cfg.keep_system);
        assert!(!cfg.simulate_user);
        assert!(!cfg.help);
        assert_eq!(cfg.timeout, None);
        assert!(cfg.command.is_empty());
    }

    #[test]
    fn display_flag() {
        let cfg = parse_from(&["-s"]).unwrap();
        assert!(cfg.keep_display);
        assert!(!cfg.keep_system);
    }

    #[test]
    fn system_flag() {
        let cfg = parse_from(&["-i"]).unwrap();
        assert!(cfg.keep_system);
        assert!(!cfg.keep_display);
    }

    #[test]
    fn simulate_flag() {
        let cfg = parse_from(&["-u"]).unwrap();
        assert!(cfg.simulate_user);
    }

    #[test]
    fn help_short_flag() {
        let cfg = parse_from(&["-h"]).unwrap();
        assert!(cfg.help);
    }

    #[test]
    fn help_long_flag() {
        let cfg = parse_from(&["--help"]).unwrap();
        assert!(cfg.help);
    }

    #[test]
    fn timeout_flag() {
        let cfg = parse_from(&["-t", "300"]).unwrap();
        assert_eq!(cfg.timeout, Some(300));
    }

    #[test]
    fn timeout_missing_value() {
        let err = parse_from(&["-t"]).unwrap_err();
        assert!(err.contains("requires"), "error was: {err}");
    }

    #[test]
    fn timeout_invalid_value() {
        let err = parse_from(&["-t", "abc"]).unwrap_err();
        assert!(err.contains("invalid"), "error was: {err}");
    }

    #[test]
    fn timeout_negative_value() {
        let err = parse_from(&["-t", "-5"]).unwrap_err();
        // "-5" starts with '-' so it's either caught as unknown flag or parse error
        assert!(
            err.contains("invalid") || err.contains("unknown"),
            "error was: {err}"
        );
    }

    #[test]
    fn unknown_flag_errors() {
        let err = parse_from(&["-z"]).unwrap_err();
        assert!(err.contains("unknown"), "error was: {err}");
    }

    #[test]
    fn unknown_long_flag_errors() {
        let err = parse_from(&["--verbose"]).unwrap_err();
        assert!(err.contains("unknown"), "error was: {err}");
    }

    #[test]
    fn all_flags_combined() {
        let cfg = parse_from(&["-s", "-i", "-u", "-t", "60"]).unwrap();
        assert!(cfg.keep_display);
        assert!(cfg.keep_system);
        assert!(cfg.simulate_user);
        assert_eq!(cfg.timeout, Some(60));
        assert!(cfg.command.is_empty());
    }

    #[test]
    fn command_capture() {
        let cfg = parse_from(&["echo", "hello", "world"]).unwrap();
        assert_eq!(cfg.command, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn flags_then_command() {
        let cfg = parse_from(&["-s", "-t", "300", "unzip", "file.zip"]).unwrap();
        assert!(cfg.keep_display);
        assert_eq!(cfg.timeout, Some(300));
        assert_eq!(cfg.command, vec!["unzip", "file.zip"]);
    }

    #[test]
    fn command_with_dashes_in_args() {
        // The command itself may have flags — once we see a non-flag token,
        // everything is captured as the command.
        let cfg = parse_from(&["-s", "curl", "-O", "https://example.com"]).unwrap();
        assert!(cfg.keep_display);
        assert_eq!(cfg.command, vec!["curl", "-O", "https://example.com"]);
    }

    #[test]
    fn timeout_zero_is_valid() {
        let cfg = parse_from(&["-t", "0"]).unwrap();
        assert_eq!(cfg.timeout, Some(0));
    }

    #[test]
    fn timeout_large_value() {
        let cfg = parse_from(&["-t", "86400"]).unwrap();
        assert_eq!(cfg.timeout, Some(86400)); // 24 hours
    }

    #[test]
    fn duplicate_flags_are_idempotent() {
        let cfg = parse_from(&["-s", "-s", "-i", "-i"]).unwrap();
        assert!(cfg.keep_display);
        assert!(cfg.keep_system);
    }

    #[test]
    fn timeout_overwritten_by_last() {
        // Second -t wins (this is natural behavior of the parser).
        let cfg = parse_from(&["-t", "100", "-t", "200"]).unwrap();
        assert_eq!(cfg.timeout, Some(200));
    }

    #[test]
    fn version_short_flag() {
        let cfg = parse_from(&["-V"]).unwrap();
        assert!(cfg.version);
    }

    #[test]
    fn version_long_flag() {
        let cfg = parse_from(&["--version"]).unwrap();
        assert!(cfg.version);
    }

    #[test]
    fn double_dash_separator() {
        let cfg = parse_from(&["-s", "--", "-weird-cmd", "arg"]).unwrap();
        assert!(cfg.keep_display);
        assert_eq!(cfg.command, vec!["-weird-cmd", "arg"]);
    }

    #[test]
    fn watch_pid_flag() {
        let cfg = parse_from(&["-w", "1234"]).unwrap();
        assert_eq!(cfg.watch_pid, Some(1234));
    }

    #[test]
    fn watch_pid_missing_value() {
        let err = parse_from(&["-w"]).unwrap_err();
        assert!(err.contains("requires"), "error was: {err}");
    }

    #[test]
    fn watch_pid_invalid_value() {
        let err = parse_from(&["-w", "abc"]).unwrap_err();
        assert!(err.contains("invalid"), "error was: {err}");
    }

    #[test]
    fn watch_pid_with_flags() {
        let cfg = parse_from(&["-s", "-w", "5678", "-t", "60"]).unwrap();
        assert!(cfg.keep_display);
        assert_eq!(cfg.watch_pid, Some(5678));
        assert_eq!(cfg.timeout, Some(60));
    }

    #[test]
    fn double_dash_with_no_command() {
        let cfg = parse_from(&["-s", "--"]).unwrap();
        assert!(cfg.keep_display);
        assert!(cfg.command.is_empty());
    }

    // ─── -d alias for -s ──────────────────────────────────────────────────

    #[test]
    fn display_alias_d_flag() {
        let cfg = parse_from(&["-d"]).unwrap();
        assert!(cfg.keep_display);
    }

    #[test]
    fn d_and_s_are_equivalent() {
        let cfg_s = parse_from(&["-s"]).unwrap();
        let cfg_d = parse_from(&["-d"]).unwrap();
        assert_eq!(cfg_s.keep_display, cfg_d.keep_display);
    }

    // ─── Human-readable durations ─────────────────────────────────────────

    #[test]
    fn duration_plain_seconds() {
        let cfg = parse_from(&["-t", "300"]).unwrap();
        assert_eq!(cfg.timeout, Some(300));
    }

    #[test]
    fn duration_minutes() {
        let cfg = parse_from(&["-t", "5m"]).unwrap();
        assert_eq!(cfg.timeout, Some(300));
    }

    #[test]
    fn duration_hours() {
        let cfg = parse_from(&["-t", "2h"]).unwrap();
        assert_eq!(cfg.timeout, Some(7200));
    }

    #[test]
    fn duration_seconds_suffix() {
        let cfg = parse_from(&["-t", "90s"]).unwrap();
        assert_eq!(cfg.timeout, Some(90));
    }

    #[test]
    fn duration_hours_and_minutes() {
        let cfg = parse_from(&["-t", "1h30m"]).unwrap();
        assert_eq!(cfg.timeout, Some(5400));
    }

    #[test]
    fn duration_hours_minutes_seconds() {
        let cfg = parse_from(&["-t", "1h30m45s"]).unwrap();
        assert_eq!(cfg.timeout, Some(5445));
    }

    #[test]
    fn duration_invalid_unit() {
        let err = parse_from(&["-t", "5x"]).unwrap_err();
        assert!(err.contains("invalid"), "error was: {err}");
    }

    #[test]
    fn duration_invalid_text() {
        let err = parse_from(&["-t", "abc"]).unwrap_err();
        assert!(err.contains("invalid"), "error was: {err}");
    }

    #[test]
    fn duration_zero_minutes() {
        let cfg = parse_from(&["-t", "0m"]).unwrap();
        assert_eq!(cfg.timeout, Some(0));
    }
}
