// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! A finished line of strace output, as produced with `--follow-forks` and
//! `--absolute-timestamps=format:unix,precision:us`:
//!
//! ```text
//! [pid  12345] 1699999999.123456 openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY|O_CLOEXEC) = 3
//! ```

use crate::syscalls::{ParseError, Syscall};
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A fully parsed line of strace output: a PID, a timestamp, and a system
/// call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The ID of the traced process that performed the call.
    ///
    /// Padded to five digits when displayed, as strace does.
    pub pid: u32,
    /// When the call was made, as a Unix time with microsecond precision.
    pub time: SystemTime,
    /// The system call and its return value.
    pub syscall: Syscall,
}

/// Parses the `[pid  12345]` prefix of a line, returning the PID and the rest
/// of the line.
fn parse_pid(input: &str) -> Result<(u32, &str), ParseError> {
    let error = || ParseError::new("pid", input);
    let (head, rest) = input.split_once(']').ok_or_else(error)?;
    let head = head.strip_prefix('[').ok_or_else(error)?;
    let head = head.strip_prefix("pid ").ok_or_else(error)?;
    let pid = head.split_whitespace().next().ok_or_else(error)?;
    let pid = pid.parse::<u32>().map_err(|_| error())?;
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

impl Line {
    /// Builds a line record from its PID, time and system call.
    #[must_use]
    pub fn from_parts(pid: u32, time: SystemTime, syscall: Syscall) -> Self {
        Self { pid, time, syscall }
    }
}

impl fmt::Display for Line {
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
        write!(f, "{}", self.syscall)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it, as strace prints it
/// with `--follow-forks` and `--absolute-timestamps=format:unix,precision:us`.
impl FromStr for Line {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let (pid, ret) = parse_pid(s)?;
        let (time, ret) = parse_time(ret)?;
        let syscall = Syscall::from_str(ret)?;
        Ok(Self { pid, time, syscall })
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
        let line = Line::from_parts(
            12345,
            epoch(1_699_999_999, 123_456),
            Syscall::from_str("open(\"/tmp\", O_RDONLY) = 3").unwrap(),
        );
        assert_eq!(
            line.to_string(),
            "[pid 12345] 1699999999.123456 open(\"/tmp\", O_RDONLY) = 3"
        );
    }

    #[test]
    fn display_small_pid() {
        let line = Line::from_parts(
            42,
            epoch(1_700_000_000, 1),
            Syscall::from_str("close(0) = 0").unwrap(),
        );
        assert_eq!(
            line.to_string(),
            "[pid    42] 1700000000.000001 close(0) = 0"
        );
    }

    #[test]
    fn parse() {
        let line =
            Line::from_str("[pid  1234] 1699999999.123456 open(\"/tmp\", O_RDONLY) = 3").unwrap();
        assert_eq!(line.pid, 1234);
        assert_eq!(line.time, epoch(1_699_999_999, 123_456));
        assert!(matches!(line.syscall, Syscall::Open(_)));
    }

    #[test]
    fn parse_generic() {
        let line = Line::from_str("[pid 1] 1700000000.000000 spill(0x1, 0x2) = 0").unwrap();
        assert_eq!(line.pid, 1);
        assert!(matches!(line.syscall, Syscall::Generic(_)));
    }

    #[test]
    fn parse_invalid() {
        assert!(Line::from_str("").is_err());
        assert!(Line::from_str("open(\"/tmp\", O_RDONLY) = 3").is_err());
        assert!(
            Line::from_str("[pid abc] 1700000000.000000 open(\"/tmp\", O_RDONLY) = 3").is_err()
        );
        assert!(Line::from_str("[pid 1] 1700000000 open(\"/tmp\", O_RDONLY) = 3").is_err());
        assert!(Line::from_str("[pid 1] 1700000000.9999999 open(\"/tmp\", O_RDONLY) = 3").is_err());
        assert!(Line::from_str("[pid 1] 1700000000.000000").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "[pid 12345] 1699999999.123456 open(\"/tmp\", O_RDONLY) = 3",
            "[pid    42] 1700000000.000001 close(0) = 0",
            "[pid  2345] 1700000000.050000 execve(\"/bin/echo\", [\"echo\"], []) = 0",
        ] {
            assert_eq!(Line::from_str(line).unwrap().to_string(), line);
        }
    }
}
