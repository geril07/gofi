//! Running an item's action: launch a process or copy a cliphist entry.
//!
//! Nothing here logs argv output or clipboard content. Errors name the program
//! and the exit status only, so a preview can never leak into a log or a notice.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Stdio};

/// Spawn a detached child. The launcher exits immediately after, so the child is
/// reparented; no stdout/stderr pipe is kept open.
pub fn spawn(argv: &[OsString], cwd: Option<&Path>) -> Result<(), String> {
    let Some((program, args)) = argv.split_first() else {
        return Err("empty command".to_string());
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not launch {}: {error}", program.to_string_lossy()))
}

/// `cliphist decode <id> | wl-copy`, without a shell and without buffering the
/// entry in this process.
pub fn copy_clipboard(cliphist: &OsStr, id: &str, wl_copy: &OsStr) -> Result<(), String> {
    let mut decode = Command::new(cliphist)
        .arg("decode")
        .arg(id)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not run {}: {error}", cliphist.to_string_lossy()))?;

    let Some(stdout) = decode.stdout.take() else {
        let _ = decode.kill();
        let _ = decode.wait();
        return Err("cliphist produced no output pipe".to_string());
    };

    let status = Command::new(wl_copy)
        .stdin(Stdio::from(stdout))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not run {}: {error}", wl_copy.to_string_lossy()));
    let _ = decode.wait();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!(
            "{} exited with {status}",
            wl_copy.to_string_lossy()
        )),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::*;
    use crate::test_support::spawn_guard;

    fn script(dir: &TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, body).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn wait_for(path: &Path) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if path.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn spawn_runs_the_argv_with_the_cwd() {
        let _guard = spawn_guard();
        let temp = TempDir::new().unwrap();
        let work = temp.path().join("work");
        fs::create_dir_all(&work).unwrap();
        let marker = temp.path().join("marker");
        let program = script(&temp, "launch.sh", "#!/bin/sh\npwd > \"$1\"\n");

        spawn(
            &[program.into_os_string(), marker.as_os_str().to_os_string()],
            Some(&work),
        )
        .unwrap();

        assert!(wait_for(&marker), "the child did not run");
        assert_eq!(
            fs::read_to_string(&marker).unwrap().trim(),
            work.to_str().unwrap()
        );
    }

    #[test]
    fn spawn_reports_a_missing_program_without_panicking() {
        let _guard = spawn_guard();
        let error = spawn(&[OsString::from("/definitely/not/a/program")], None).unwrap_err();
        assert!(error.contains("could not launch"), "{error}");
    }

    #[test]
    fn copy_clipboard_pipes_decode_into_wl_copy() {
        let _guard = spawn_guard();
        let temp = TempDir::new().unwrap();
        let captured = temp.path().join("captured");
        let cliphist = script(
            &temp,
            "cliphist",
            "#!/bin/sh\n[ \"$1\" = decode ] && [ \"$2\" = 42 ] || exit 9\nprintf 'the entry'\n",
        );
        let wl_copy = script(
            &temp,
            "wl-copy",
            &format!("#!/bin/sh\ncat > '{}'\n", captured.display()),
        );

        copy_clipboard(cliphist.as_os_str(), "42", wl_copy.as_os_str()).unwrap();
        assert_eq!(fs::read_to_string(&captured).unwrap(), "the entry");
    }

    #[test]
    fn copy_clipboard_reports_a_failed_copy() {
        let _guard = spawn_guard();
        let temp = TempDir::new().unwrap();
        let cliphist = script(&temp, "cliphist", "#!/bin/sh\nprintf 'x'\n");
        let wl_copy = script(&temp, "wl-copy", "#!/bin/sh\nexit 4\n");

        let error = copy_clipboard(cliphist.as_os_str(), "1", wl_copy.as_os_str()).unwrap_err();
        assert!(error.contains("exited with"), "{error}");
    }
}
