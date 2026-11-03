//! Observe separately launched games without blocking the library event loop.
use std::process::Child;
#[derive(Default)]
pub(super) struct Poll {
    pub exited: bool,
    pub errors: Vec<String>,
}
pub(super) fn poll(children: &mut Vec<(String, Child)>) -> Poll {
    let mut result = Poll::default();
    children.retain_mut(|(id, child)| match child.try_wait() {
        Ok(Some(status)) => {
            result.exited = true;
            if !status.success() {
                result.errors.push(match status.code() {
                    Some(code) => format!("Game {id} stopped unexpectedly (exit code {code})."),
                    None => format!("Game {id} stopped unexpectedly ({status})."),
                });
                if let Ok(Some(detail)) = crate::registry::default_root()
                    .and_then(|root| crate::takedawn::last_launch_error(&root, id))
                {
                    result.errors.push(detail);
                }
            }
            false
        }
        Ok(None) => true,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::Interrupted {
                result.errors.push(format!(
                    "Unable to check the game process for {id}: {error}"
                ));
            }
            true
        }
    });
    result
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    fn finished(code: u8) -> Child {
        let mut child = Command::new("sh")
            .args(["-c", &format!("exit {code}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.wait().unwrap();
        child
    }
    #[test]
    fn normal_quit_is_reaped_without_a_failure() {
        let mut children = vec![("com.test.normal".into(), finished(0))];
        let result = poll(&mut children);
        assert!(result.exited);
        assert!(result.errors.is_empty());
        assert!(children.is_empty());
    }
    #[test]
    fn failed_start_reports_the_game_and_exit_code() {
        let mut children = vec![("com.test.failed".into(), finished(7))];
        let result = poll(&mut children);
        assert!(result.exited);
        assert_eq!(
            result.errors,
            ["Game com.test.failed stopped unexpectedly (exit code 7)."]
        );
        assert!(children.is_empty());
    }
    #[test]
    fn terminated_game_reports_failure_and_releases_its_process_handle() {
        let mut child = Command::new("sh")
            .args(["-c", "read token; exit 0"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.kill().unwrap();
        assert!(child.wait().unwrap().code().is_none());
        let mut children = vec![("terminated-game".into(), child)];
        let result = poll(&mut children);
        assert!(result.exited);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("Game terminated-game stopped unexpectedly"));
        assert!(children.is_empty());
    }
    #[test]
    fn successful_exits_do_not_hide_concurrent_failures() {
        let mut children = vec![
            ("good-first".into(), finished(0)),
            ("bad-one".into(), finished(7)),
            ("good-middle".into(), finished(0)),
            ("bad-two".into(), finished(9)),
            ("good-last".into(), finished(0)),
        ];
        let result = poll(&mut children);
        assert!(result.exited);
        assert_eq!(result.errors.len(), 2);
        assert!(result.errors[0].contains("bad-one") && result.errors[0].contains('7'));
        assert!(result.errors[1].contains("bad-two") && result.errors[1].contains('9'));
        assert!(children.is_empty());
    }
    #[test]
    fn running_process_remains_tracked_until_completion() {
        let child = Command::new("sh")
            .args(["-c", "read token; exit 0"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut children = vec![("waiting".into(), child)];
        let writer = children[0].1.stdin.take().unwrap();
        let pending = poll(&mut children);
        assert!(!pending.exited && pending.errors.is_empty());
        assert_eq!(children.len(), 1);
        drop(writer);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let result = poll(&mut children);
            assert!(result.errors.is_empty());
            if result.exited {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(children.is_empty());
    }
}
