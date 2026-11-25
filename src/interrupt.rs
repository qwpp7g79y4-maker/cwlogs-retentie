//! SIGINT (Ctrl-C) ends the run with exit 130 and one line on standard error.
//!
//! The binary writes its report in one piece at the very end, so a signal before that
//! leaves standard output empty. While that write runs, the signal is only noted and the
//! report is finished, so a report is never cut in half. Only Unix has SIGINT; elsewhere
//! nothing is installed. This is binary-only code: the library forbids `unsafe`.

use std::sync::atomic::{AtomicBool, Ordering};

static WRITING: AtomicBool = AtomicBool::new(false);

/// Install the handler. Call first thing in `main`.
pub fn install() {
    imp::install();
}

/// Mark the start (`true`) and the end (`false`) of the final write of the report.
pub fn set_writing(writing: bool) {
    WRITING.store(writing, Ordering::SeqCst);
}

#[cfg(unix)]
mod imp {
    use super::{Ordering, WRITING};

    /// Exit status of a run ended by SIGINT.
    const EXIT_INTERRUPTED: u8 = 130;

    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
        #[link_name = "write"]
        fn write_fd(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize;
        fn _exit(status: i32) -> !;
    }
    const SIGINT: i32 = 2;
    const SIG_DFL: usize = 0;
    const MESSAGE: &[u8] = b"cwlogs-retentie: interrupted by SIGINT\n";

    extern "C" fn on_sigint(_signum: i32) {
        // A report that is being written is finished first; a second Ctrl-C then has the
        // default effect. Only async-signal-safe calls are used here.
        // SAFETY: `signal`, `write`, and `_exit` are async-signal-safe and get valid arguments.
        unsafe {
            if WRITING.load(Ordering::SeqCst) {
                signal(SIGINT, SIG_DFL);
                return;
            }
            let _ = write_fd(2, MESSAGE.as_ptr().cast(), MESSAGE.len());
            _exit(i32::from(EXIT_INTERRUPTED));
        }
    }

    pub fn install() {
        // SAFETY: the handler is an `extern "C" fn(i32)` that only calls async-signal-safe functions.
        unsafe {
            signal(SIGINT, on_sigint as extern "C" fn(i32) as usize);
        }
    }
}

#[cfg(not(unix))]
mod imp {
    pub fn install() {}
}
