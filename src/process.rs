// Watch an external process by PID.
//
// Uses OpenProcess + WaitForSingleObject to check if a process is still alive
// without owning it. This implements caffeinate's -w flag.

use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

const SYNCHRONIZE: u32 = 0x00100000;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

/// Check if a process with the given PID is still running.
/// Returns false if the process has exited or can't be opened.
pub fn is_alive(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false; // Can't open — process doesn't exist or access denied
        }

        // Wait with 0 timeout = non-blocking check.
        let result = WaitForSingleObject(handle, 0);
        CloseHandle(handle);

        // WAIT_TIMEOUT means the process is still running.
        result == WAIT_TIMEOUT
    }
}
