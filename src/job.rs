// Process-tree management via Win32 Job Objects.
//
// When a child process is assigned to a Job Object with
// JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, the entire process tree
// (child + all grandchildren) is killed when the job handle is closed.
// This ensures that `buzz -t 60 cmd /C "long_task.exe"` kills long_task.exe
// too, not just cmd.exe.

use std::process::Child;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::OpenProcess;

const PROCESS_SET_QUOTA: u32 = 0x0100;
const PROCESS_TERMINATE: u32 = 0x0001;

/// A Win32 Job Object that kills all assigned processes when dropped.
pub struct JobObject {
    handle: HANDLE,
}

impl JobObject {
    /// Create a new anonymous Job Object configured to kill all processes on close.
    pub fn new() -> Option<Self> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return None;
            }

            // Configure: kill all processes when the job handle is closed.
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if ok == 0 {
                CloseHandle(handle);
                return None;
            }

            Some(JobObject { handle })
        }
    }

    /// Assign a child process to this job. All processes spawned by the child
    /// will also belong to the job and be killed when the job is dropped.
    pub fn assign_child(&self, child: &Child) -> bool {
        unsafe {
            let pid = child.id();
            let process_handle = OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE,
                0, // bInheritHandle = false
                pid,
            );
            if process_handle.is_null() {
                return false;
            }

            let ok = AssignProcessToJobObject(self.handle, process_handle);
            CloseHandle(process_handle);
            ok != 0
        }
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        // Closing the job handle kills all assigned processes
        // (due to JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE).
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
