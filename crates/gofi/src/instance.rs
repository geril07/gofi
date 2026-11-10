//! One launcher at a time.
//!
//! The first process holds an advisory lock in `$XDG_RUNTIME_DIR`; a second
//! invocation reads the holder's PID and asks it to close, which is the
//! accepted "pressing the keybind again toggles it closed" behavior. The lock
//! is advisory and released by the kernel if the holder dies.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

use fs2::FileExt;

pub enum Claim {
    /// This process owns the lock for as long as the guard lives.
    Owner(Guard),
    /// Another gofi holds the lock. Its PID is `None` if the file was unreadable.
    AlreadyRunning { pid: Option<u32> },
}

pub struct Guard {
    _file: Option<File>,
}

impl Guard {
    fn none() -> Self {
        Self { _file: None }
    }
}

pub fn claim(runtime_dir: Option<&Path>) -> Claim {
    let Some(dir) = runtime_dir else {
        // No runtime dir means no single-instance promise; still run.
        return Claim::Owner(Guard::none());
    };
    let path = dir.join("gofi.lock");
    let mut file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
    {
        Ok(file) => file,
        Err(_) => return Claim::Owner(Guard::none()),
    };

    match file.try_lock_exclusive() {
        Ok(()) => {
            // A stale PID from a crashed run is harmless: the lock was free.
            let _ = file.set_len(0);
            let _ = writeln!(file, "{}", std::process::id());
            let _ = file.flush();
            Claim::Owner(Guard { _file: Some(file) })
        }
        Err(error) if error.kind() == ErrorKind::WouldBlock => Claim::AlreadyRunning {
            pid: read_pid(&path),
        },
        // Locking is a nicety; a filesystem without flock should not stop the UI.
        Err(_) => Claim::Owner(Guard::none()),
    }
}

fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Ask another gofi to close. A stale PID reused by an unrelated program is
/// ignored by checking `/proc/<pid>/comm` first.
pub fn terminate_if_gofi(pid: u32) {
    if !is_gofi(pid) {
        return;
    }
    // SAFETY: `kill` is safe to call with any PID; the comm check above keeps a
    // reused PID from being signalled by mistake.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
}

fn is_gofi(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|comm| comm.trim() == "gofi")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn a_second_claim_reports_the_holder_then_succeeds_after_drop() {
        let temp = TempDir::new().unwrap();
        let first = match claim(Some(temp.path())) {
            Claim::Owner(guard) => guard,
            Claim::AlreadyRunning { .. } => panic!("first claim should own the lock"),
        };

        match claim(Some(temp.path())) {
            Claim::AlreadyRunning { pid } => {
                assert_eq!(pid, Some(std::process::id()));
            }
            Claim::Owner(_) => panic!("second claim must not own the lock"),
        }

        drop(first);
        // A concurrently spawned child can inherit the lock's open file
        // description for a moment before its exec; the lock then clears.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if matches!(claim(Some(temp.path())), Claim::Owner(_)) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the lock was not released"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn no_runtime_dir_still_runs() {
        assert!(matches!(claim(None), Claim::Owner(_)));
    }

    #[test]
    fn unrelated_processes_are_not_signalled() {
        let _guard = crate::test_support::spawn_guard();
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        assert!(!is_gofi(child.id()));
        terminate_if_gofi(child.id());

        // Still alive: the comm check refused to signal it.
        let status = child.try_wait().unwrap();
        assert!(status.is_none(), "an unrelated process was terminated");

        let _ = child.kill();
        let _ = child.wait();
    }
}
