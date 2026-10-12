// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Reconstruction of system calls that `strace` splits across multiple lines.
//!
//! When `strace` traces more than one process or thread at a time, a call
//! that is still running when another traced process emits output is printed
//! in two pieces: the beginning of the call ends with `<unfinished ...>`, and
//! its end begins with `<... name resumed>`. For example:
//!
//! ```text
//! 394527 1791802869.793329 wait4(-1 <unfinished ...>
//! 394528 1791802869.793397 set_robust_list(0x7f41 <unfinished ...>
//! 394527 1791802869.793397 <... wait4 resumed>, 0x7fff, WNOHANG, NULL) = 0
//! ```
//!
//! [`Reconstructor`] keeps the unfinished piece of every pid and joins it
//! with the matching resumed line, yielding the same single [`Event`] that a
//! call printed on one line would produce. The event keeps the pid and the
//! timestamp of the unfinished line, which is when the call was entered, as
//! `strace` reports for calls that are not split.
//!
//! Because events are emitted when their resumed line is read, they are not
//! necessarily ordered by timestamp: the entry time of a call that started
//! early can precede the entry time of a call that completed in between.
//!
//! A resumed call whose merged text the syscall decoder cannot read is
//! dropped, exactly as the same call would be if `strace` had printed it on
//! one line. This includes calls that resume with an indeterminate result,
//! printed as `= ?`, such as restarted `rt_sigsuspend(2)` calls.

use crate::Event;
use crate::syscalls::Syscall;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::SystemTime;

/// The trailing marker `strace` writes when a call is not finished.
const UNFINISHED: &str = " <unfinished ...>";
/// The prefix and suffix `strace` writes around the name of a resumed call.
const RESUMED_OPEN: &str = "<... ";
const RESUMED_CLOSE: &str = " resumed>";

/// One line of `strace` output, seen as part of a possibly multi-line call.
enum Fragment {
    /// A call that fits in a single line and is already complete.
    Complete(Event),
    /// The beginning of a call, up to the `<unfinished ...>` marker.
    Unfinished {
        pid: u32,
        time: SystemTime,
        prefix: String,
    },
    /// The end of a call, after the `<... name resumed>` marker.
    Resumed { pid: u32, suffix: String },
}

impl Fragment {
    /// Classifies a line, using the syscall decoder itself for complete calls.
    fn parse(line: &str) -> Option<Self> {
        let (pid, time, rest) = Event::split_prefix(line).ok()?;
        if let Some(prefix) = rest.strip_suffix(UNFINISHED) {
            return Some(Self::Unfinished {
                pid,
                time,
                prefix: prefix.to_string(),
            });
        }
        if let Some(rest) = rest.strip_prefix(RESUMED_OPEN)
            && let Some((_name, suffix)) = rest.split_once(RESUMED_CLOSE)
        {
            return Some(Self::Resumed {
                pid,
                suffix: suffix.to_string(),
            });
        }
        Event::from_str(line).ok().map(Self::Complete)
    }
}

/// Reconstructs the [`Event`]s of a trace, joining the lines of the calls
/// that `strace` splits within a single event.
///
/// Feed lines in the order they were read with [`Reconstructor::push`]; a
/// complete call is returned right away, while an unfinished call is kept
/// until its resumed line arrives. Lines that are not system calls, such as
/// the `+++ exited ... +++` messages, are ignored.
///
/// The unfinished piece of a pid that never resumes, because its process died
/// mid-call, stays in memory; it is bounded by the number of such pids.
#[derive(Debug, Default)]
pub(crate) struct Reconstructor {
    unfinished: HashMap<u32, (SystemTime, String)>,
}

impl Reconstructor {
    /// Builds a reconstructor with no unfinished call.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Feeds one line of `strace` output, returning the [`Event`] it
    /// completes.
    ///
    /// A complete, single-line call is returned as it is. A `<... name
    /// resumed>` line is joined with the `<unfinished ...>` line of the same
    /// pid that preceded it, producing the complete event. An unfinished line
    /// stores its beginning and yields `None`; so do lines that are not
    /// processed into system calls, including a resumed line whose beginning
    /// was never seen.
    pub(crate) fn push(&mut self, line: &str) -> Option<Event> {
        match Fragment::parse(line)? {
            Fragment::Complete(event) => Some(event),
            Fragment::Unfinished { pid, time, prefix } => {
                self.unfinished.insert(pid, (time, prefix));
                None
            }
            Fragment::Resumed { pid, suffix } => {
                let (time, prefix) = self.unfinished.remove(&pid)?;
                let syscall = Syscall::from_str(&format!("{prefix}{suffix}")).ok()?;
                Some(Event::from_parts(pid, time, syscall))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{Duration, UNIX_EPOCH};

    fn epoch(secs: u64, micros: u32) -> SystemTime {
        UNIX_EPOCH + Duration::new(secs, micros * 1000)
    }

    #[test]
    fn complete_line() {
        let mut reconstructor = Reconstructor::new();
        let event = reconstructor
            .push("394527 1700000000.000000 close(0) = 0")
            .unwrap();
        assert_eq!(event.pid, 394527);
        assert_eq!(event.time, epoch(1_700_000_000, 0));
        assert!(matches!(event.syscall, Syscall::Close(_)));
    }

    #[test]
    fn unfinished_is_not_emitted() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("394527 1791802869.793329 wait4(-1 <unfinished ...>")
                .is_none()
        );
    }

    #[test]
    fn unfinished_then_resumed() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("394527 1791802869.793329 mystery(0x1, 2 <unfinished ...>")
                .is_none()
        );
        let event = reconstructor
            .push("394527 1791802869.793397 <... mystery resumed>, -3, \"a\") = 0")
            .unwrap();
        assert_eq!(event.pid, 394527);
        // The event keeps the time of the unfinished line, when the call was
        // entered, which is what strace reports for calls that are not split.
        assert_eq!(event.time, epoch(1_791_802_869, 793_329));
        assert_eq!(event.syscall.to_string(), "mystery(0x1, 2, -3, \"a\") = 0");
    }

    #[test]
    fn dedicated_type_is_reconstructed() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("1 1700000000.000000 openat(AT_FDCWD, \"/tmp\", O_RDONLY <unfinished ...>")
                .is_none()
        );
        let event = reconstructor
            .push("1 1700000000.000100 <... openat resumed>) = 3")
            .unwrap();
        assert!(matches!(event.syscall, Syscall::Openat(_)));
        assert_eq!(
            event.syscall.to_string(),
            "openat(AT_FDCWD, \"/tmp\", O_RDONLY) = 3"
        );
    }

    #[test]
    fn interleaved_pids() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("1 1700000000.000001 openat(AT_FDCWD, \"/tmp\", O_RDONLY <unfinished ...>")
                .is_none()
        );
        // Another pid completes in between; its line is returned right away.
        let other = reconstructor
            .push("2 1700000000.000002 close(0) = 0")
            .unwrap();
        assert_eq!(other.pid, 2);
        assert!(matches!(other.syscall, Syscall::Close(_)));
        // The first pid resumes and is joined with its own unfinished line.
        let event = reconstructor
            .push("1 1700000000.000003 <... openat resumed>) = 3")
            .unwrap();
        assert_eq!(event.pid, 1);
        assert!(matches!(event.syscall, Syscall::Openat(_)));
        assert_eq!(event.time, epoch(1_700_000_000, 1));
    }

    #[test]
    fn resumed_without_unfinished() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("1 1700000000.000000 <... openat resumed>) = 3")
                .is_none()
        );
    }

    #[test]
    fn unparseable_resumed_is_dropped() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("1 1700000000.000000 rt_sigsuspend([], 8 <unfinished ...>")
                .is_none()
        );
        assert!(
            reconstructor
                .push(
                    "1 1700000000.000100 <... rt_sigsuspend resumed>) = ? \
                     ERESTARTNOHAND (To be restarted if no handler)"
                )
                .is_none()
        );
    }

    #[test]
    fn non_syscall_lines_are_ignored() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("1 1700000000.000000 +++ exited with 0 +++")
                .is_none()
        );
        assert!(
            reconstructor
                .push(
                    "1 1700000000.000000 --- SIGCHLD {si_signo=SIGCHLD, si_code=CLD_EXITED, \
                     si_pid=2, si_status=0} ---"
                )
                .is_none()
        );
        assert!(reconstructor.push("not a trace line").is_none());
        assert!(reconstructor.push("").is_none());
    }

    #[test]
    fn bracketed_pid_form() {
        let mut reconstructor = Reconstructor::new();
        assert!(
            reconstructor
                .push("[pid  1234] 1791802869.793329 mystery(0x1 <unfinished ...>")
                .is_none()
        );
        let event = reconstructor
            .push("[pid  1234] 1791802869.793397 <... mystery resumed>, 0x2) = 0")
            .unwrap();
        assert_eq!(event.pid, 1234);
        assert_eq!(event.syscall.to_string(), "mystery(0x1, 0x2) = 0");
    }

    #[test]
    fn display_round_trip() {
        let mut reconstructor = Reconstructor::new();
        reconstructor.push("1 1700000000.000000 mystery(0x1 <unfinished ...>");
        let event = reconstructor
            .push("1 1700000000.000100 <... mystery resumed>, 2, \"a\") = -1 ENOENT")
            .unwrap();
        let reparsed: Event = event.to_string().parse().expect("reparse displayed event");
        assert_eq!(event, reparsed);
    }

    /// Returns whether `strace` can trace forks in this environment.
    fn strace_can_trace_forks() -> bool {
        let out = Command::new("strace")
            .args([
                "--absolute-timestamps=format:unix,precision:us",
                "--follow-forks",
                "sh",
                "-c",
                "sleep 0.02 & wait",
            ])
            .output();
        match out {
            Ok(out) => String::from_utf8_lossy(&out.stderr).contains("[pid "),
            Err(_) => false,
        }
    }

    /// Runs `cmd` under `strace`, capturing the raw trace to a temporary file.
    fn raw_trace(cmd: &[&str]) -> String {
        // Tests run in parallel threads of the same process, so the path must
        // be unique per call, not just per process.
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "restrace-reconstruct-{}-{n}.log",
            std::process::id()
        ));
        let status = Command::new("strace")
            .args([
                "--absolute-timestamps=format:unix,precision:us",
                "--follow-forks",
                "-o",
            ])
            .arg(&path)
            .args(cmd)
            .status()
            .expect("run strace");
        assert!(status.success(), "strace failed");
        let text = std::fs::read_to_string(&path).expect("read the raw trace");
        std::fs::remove_file(&path).expect("remove the temporary trace");
        text
    }

    /// Reconstructs a real trace and checks that it yields more events than
    /// the complete single lines alone, on the same input.
    #[test]
    fn reconstructs_real_split_calls() {
        if !strace_can_trace_forks() {
            return;
        }
        // Several processes doing file calls at the same time make strace
        // split many of them into an `<unfinished ...>` line and a
        // `<... name resumed>` line.
        let text = raw_trace(&[
            "sh",
            "-c",
            "for i in 1 2 3 4 5 6 7 8; do ls /usr/bin >/dev/null & done; wait",
        ]);
        let complete = text
            .lines()
            .filter(|line| line.parse::<Event>().is_ok())
            .count();
        let mut reconstructor = Reconstructor::new();
        let events: Vec<Event> = text
            .lines()
            .filter_map(|line| reconstructor.push(line))
            .collect();

        assert!(complete > 0, "expected some complete syscalls");
        assert!(
            events.len() > complete,
            "expected the split calls to be reconstructed: \
             {complete} complete lines became {} events",
            events.len()
        );
        for event in events {
            let reparsed: Event = event.to_string().parse().expect("reparse displayed event");
            assert_eq!(event, reparsed);
        }
    }
}
