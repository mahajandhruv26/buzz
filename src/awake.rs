// Windows sleep-prevention via SetThreadExecutionState.
//
// Flags reference (from Win32 docs):
//   ES_CONTINUOUS            0x80000000  — keep the state until next call
//   ES_SYSTEM_REQUIRED       0x00000001  — prevent system sleep
//   ES_DISPLAY_REQUIRED      0x00000002  — prevent display off
//   ES_AWAYMODE_REQUIRED     0x00000040  — (not used here)

use windows_sys::Win32::System::Power::SetThreadExecutionState;

// Re-export the constants so the rest of the crate doesn't need to know
// about windows_sys directly.
pub const ES_CONTINUOUS: u32 = 0x80000000;
pub const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
pub const ES_DISPLAY_REQUIRED: u32 = 0x00000002;

/// Build the combined flag value from the user's options.
/// If neither display nor system is requested we default to system-only,
/// matching caffeinate's default behaviour.
pub fn build_flags(display: bool, system: bool) -> u32 {
    let mut flags = ES_CONTINUOUS;
    if display {
        flags |= ES_DISPLAY_REQUIRED;
    }
    if system || !display {
        // Default to system-required when no explicit choice is made.
        flags |= ES_SYSTEM_REQUIRED;
    }
    flags
}

/// Apply the execution state flags. Safe to call repeatedly.
pub fn set(flags: u32) {
    unsafe {
        SetThreadExecutionState(flags);
    }
}

/// Restore normal sleep behaviour by clearing all flags.
pub fn clear() {
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
    }
}

// ─── Unit Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_no_flags_gives_system_only() {
        // When neither display nor system is explicitly requested,
        // we default to ES_SYSTEM_REQUIRED.
        let flags = build_flags(false, false);
        assert_eq!(flags, ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
    }

    #[test]
    fn display_only() {
        let flags = build_flags(true, false);
        assert_eq!(flags, ES_CONTINUOUS | ES_DISPLAY_REQUIRED);
        // System should NOT be set when display is explicitly chosen.
        assert_eq!(flags & ES_SYSTEM_REQUIRED, 0);
    }

    #[test]
    fn system_only() {
        let flags = build_flags(false, true);
        assert_eq!(flags, ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        assert_eq!(flags & ES_DISPLAY_REQUIRED, 0);
    }

    #[test]
    fn both_display_and_system() {
        let flags = build_flags(true, true);
        assert_eq!(
            flags,
            ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
        );
    }

    #[test]
    fn continuous_always_set() {
        // ES_CONTINUOUS must always be present regardless of options.
        for display in [true, false] {
            for system in [true, false] {
                let flags = build_flags(display, system);
                assert_ne!(
                    flags & ES_CONTINUOUS,
                    0,
                    "ES_CONTINUOUS missing for display={display}, system={system}"
                );
            }
        }
    }

    #[test]
    fn set_and_clear_do_not_panic() {
        // Smoke test: calling the Windows API should not crash.
        let flags = build_flags(false, true);
        set(flags);
        clear();
    }
}
