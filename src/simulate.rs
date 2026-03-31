// Simulate user activity to prevent the OS idle timer from kicking in.
//
// We send a harmless VK_NONAME (0xFC) key-down/key-up via SendInput.
// This key has no visible effect but counts as user activity for the
// Windows idle detector.
//
// The nudge is rate-limited: it fires at most once every 30-60 seconds
// (randomised to look more natural).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP,
};

/// Epoch-second of the last nudge, shared across calls.
static LAST_NUDGE: AtomicU64 = AtomicU64::new(0);

/// Minimum interval in seconds between nudges.
const MIN_INTERVAL: u64 = 30;

/// Simple pseudo-random jitter (0–30 s) so the interval is 30–60 s.
fn jitter() -> u64 {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    t % 31
}

/// Send a harmless keystroke if enough time has passed since the last one.
pub fn nudge() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Note: load+store is not a single atomic operation, so two threads could
    // both pass the check and fire. This is safe because buzz is single-threaded;
    // nudge() is only called from the main loop. If the design ever goes
    // multi-threaded, replace with compare_exchange.
    let prev = LAST_NUDGE.load(Ordering::Relaxed);
    let interval = MIN_INTERVAL + jitter();
    if now.saturating_sub(prev) < interval {
        return;
    }

    LAST_NUDGE.store(now, Ordering::Relaxed);

    // VK_NONAME (0xFC) — a virtual key code that maps to nothing visible.
    let vk: u16 = 0xFC;

    let key_down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let key_up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    unsafe {
        let inputs = [key_down, key_up];
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }

    println!("[buzz] Simulated user activity (nudge).");
}
