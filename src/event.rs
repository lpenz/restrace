// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! A finished line of strace output, as produced with
//! `--follow-forks` and `--absolute-timestamps=format:unix,precision:us`.
//! Every line carries the pid of the traced process:
//!
//! ```text
//! 251751 1699999999.123456 openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY|O_CLOEXEC) = 3
//! 251751 1699999999.123999 +++ exited with 0 +++
//! ```
//!
//! A line records one [`Action`]: a system call the process made, or the
//! [`Exit`](crate::Exit) that terminated it.
//!
//! strace only prints the bare `<pid> ` prefix when it is tracing a command
//! with `--follow-forks` and redirecting the trace with `-o`, as done by
//! [`crate::tokio::EventStream`]; without it, lines of the top-level process
//! have no pid and are not parsed into [`Event`]s.
//!
//! The bracketed `[pid  251751]` form is also accepted when parsing, and is how
//! [`fmt::Display`] renders lines.

use crate::action::Action;
use crate::syscalls::ParseError;
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A fully parsed line of strace output: a PID, a timestamp, and the
/// [`Action`] the process took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The ID of the traced process that performed the call.
    ///
    /// Padded to five digits when displayed, as strace does.
    pub pid: u32,
    /// When the call was made, as a Unix time with microsecond precision.
    pub time: SystemTime,
    /// The system call the process made, or its exit.
    pub action: Action,
}

/// Parses the pid prefix of a line, in either of the two forms strace prints:
/// the bracketed `[pid  12345]` and, with `-o FILE`, the bare `12345 `. Returns
/// the PID and the rest of the line, or an error if there is no pid.
///
/// We assume the trace was redirected with `-o`, so that `strace` prints a pid
/// on every line, including those of the top-level process; lines without a
/// pid are rejected.
fn parse_pid(input: &str) -> Result<(u32, &str), ParseError> {
    let error = || ParseError::new("pid", input);
    if let Some((head, rest)) = input.split_once(']')
        && let Some(head) = head
            .strip_prefix('[')
            .and_then(|head| head.strip_prefix("pid "))
    {
        let pid = head.split_whitespace().next().ok_or_else(error)?;
        let pid = pid.parse::<u32>().map_err(|_| error())?;
        return Ok((pid, rest.trim_start()));
    }
    // Bare form: `<pid> <secs>.<micros> ...`. The pid and the timestamp are
    // both numeric, but only the timestamp contains a dot.
    let Some((head, rest)) = input.split_once(' ') else {
        return Err(error());
    };
    if head.contains('.') || head.is_empty() {
        return Err(error());
    }
    let pid = head.parse::<u32>().map_err(|_| error())?;
    Ok((pid, rest.trim_start()))
}

/// Parses the `<secs>.<micros> ` timestamp of a line, returning the time and
/// the rest of the line.
fn parse_time(input: &str) -> Result<(SystemTime, &str), ParseError> {
    let error = || ParseError::new("timestamp", input);
    let (time_str, rest) = input.split_once(' ').ok_or_else(error)?;
    let (secs_str, micros_str) = time_str.split_once('.').ok_or_else(error)?;
    let secs = secs_str.parse::<u64>().map_err(|_| error())?;
    let micros = micros_str.parse::<u32>().map_err(|_| error())?;
    if micros >= 1_000_000 {
        return Err(error());
    }
    Ok((
        UNIX_EPOCH + Duration::new(secs, micros * 1000),
        rest.trim_start(),
    ))
}

impl Event {
    /// Builds a line record from its PID, time and action.
    #[must_use]
    pub fn from_parts(pid: u32, time: SystemTime, action: Action) -> Self {
        Self { pid, time, action }
    }

    /// Splits a line into its pid, its time and the remaining action text.
    ///
    /// This is the part of [`FromStr`] that does not depend on the action
    /// itself, shared with the reconstruction of multi-line calls.
    pub(crate) fn split_prefix(s: &str) -> Result<(u32, SystemTime, &str), ParseError> {
        let (pid, ret) = parse_pid(s.trim())?;
        let (time, ret) = parse_time(ret)?;
        Ok((pid, time, ret))
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[pid {:5}] ", self.pid)?;
        let since_epoch = self
            .time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| fmt::Error)?;
        write!(
            f,
            "{}.{:06} ",
            since_epoch.as_secs(),
            since_epoch.subsec_micros()
        )?;
        write!(f, "{}", self.action)
    }
}

/// Parses a line as [`fmt::Display`] writes it, as strace prints it
/// with `--follow-forks`, `--absolute-timestamps=format:unix,precision:us`
/// and a trace redirected with `-o`.
impl FromStr for Event {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let (pid, time, ret) = Self::split_prefix(s)?;
        let action = Action::from_str(ret)?;
        Ok(Self { pid, time, action })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch(secs: u64, micros: u32) -> SystemTime {
        UNIX_EPOCH + Duration::new(secs, micros * 1000)
    }

    #[test]
    fn display() {
        let line = Event::from_parts(
            12345,
            epoch(1_699_999_999, 123_456),
            Action::from_str("open(\"/tmp\", O_RDONLY) = 3").unwrap(),
        );
        assert_eq!(
            line.to_string(),
            "[pid 12345] 1699999999.123456 open(\"/tmp\", O_RDONLY) = 3"
        );
    }

    #[test]
    fn display_small_pid() {
        let line = Event::from_parts(
            42,
            epoch(1_700_000_000, 1),
            Action::from_str("close(0) = 0").unwrap(),
        );
        assert_eq!(
            line.to_string(),
            "[pid    42] 1700000000.000001 close(0) = 0"
        );
    }

    #[test]
    fn parse() {
        let line =
            Event::from_str("[pid  1234] 1699999999.123456 open(\"/tmp\", O_RDONLY) = 3").unwrap();
        assert_eq!(line.pid, 1234);
        assert_eq!(line.time, epoch(1_699_999_999, 123_456));
        assert!(matches!(line.action, Action::Open(_)));
    }

    #[test]
    fn parse_generic() {
        let line = Event::from_str("[pid 1] 1700000000.000000 spill(0x1, 0x2) = 0").unwrap();
        assert_eq!(line.pid, 1);
        assert!(matches!(line.action, Action::Generic(_)));
    }

    #[test]
    fn parse_bare_pid() {
        let line =
            Event::from_str("251751 1700000000.000000 execve(\"/bin/echo\", [\"echo\"], []) = 0")
                .unwrap();
        assert_eq!(line.pid, 251751);
        assert_eq!(line.time, epoch(1_700_000_000, 0));
        assert!(matches!(line.action, Action::Execve(_)));
    }

    #[test]
    fn parse_bare_pid_padded() {
        let line = Event::from_str("123  1700000000.000000 close(0) = 0").unwrap();
        assert_eq!(line.pid, 123);
        assert!(matches!(line.action, Action::Close(_)));
    }

    #[test]
    fn parse_exit() {
        let line = Event::from_str("123 1700000000.000000 +++ exited with 3 +++").unwrap();
        assert_eq!(line.pid, 123);
        assert_eq!(line.time, epoch(1_700_000_000, 0));
        assert_eq!(line.action, Action::Exit(crate::Exit::Code(3)));
        let killed = Event::from_str("123 1700000000.000000 +++ killed by SIGKILL +++").unwrap();
        assert!(matches!(killed.action, Action::Exit(_)));
    }

    #[test]
    fn parse_invalid() {
        assert!(Event::from_str("").is_err());
        assert!(Event::from_str("open(\"/tmp\", O_RDONLY) = 3").is_err());
        assert!(Event::from_str("1700000000.000000 execve(\"/bin/echo\", []) = 0").is_err());
        assert!(
            Event::from_str("[pid abc] 1700000000.000000 open(\"/tmp\", O_RDONLY) = 3").is_err()
        );
        assert!(Event::from_str("[pid 1] 1700000000 open(\"/tmp\", O_RDONLY) = 3").is_err());
        assert!(
            Event::from_str("[pid 1] 1700000000.9999999 open(\"/tmp\", O_RDONLY) = 3").is_err()
        );
        assert!(Event::from_str("[pid 1] 1700000000.000000").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "[pid 12345] 1699999999.123456 open(\"/tmp\", O_RDONLY) = 3",
            "[pid    42] 1700000000.000001 close(0) = 0",
            "[pid  2345] 1700000000.050000 execve(\"/bin/echo\", [\"echo\"], []) = 0",
            "[pid  2345] 1700000000.060000 +++ exited with 0 +++",
            "[pid  2345] 1700000000.070000 +++ killed by SIGKILL +++",
        ] {
            assert_eq!(Event::from_str(line).unwrap().to_string(), line);
        }
    }
}
