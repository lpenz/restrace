// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `vfork` system call.
//!
//! [man7]: https://man7.org/linux/man-pages/man2/vfork.2.html

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `vfork(2)` system call.
///
/// ```text
/// pid_t vfork(void);
/// ```
///
/// Strace typically outputs `vfork() = pid` or `vfork() = -1 errno`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vfork {
    /// Return value: child PID in the parent, 0 in the child, or -1 on error.
    pub result: super::fork::ForkResult,
}

impl Vfork {
    /// Builds a `vfork(2)` record from its raw return value.
    #[must_use]
    pub fn from_raw(raw_result: i64) -> Self {
        Self {
            result: super::fork::ForkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Vfork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vfork() = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it: `vfork() = ...`.
impl FromStr for Vfork {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("vfork call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args_trim = args.strip_prefix("vfork(").ok_or_else(syntax)?;
        if !args_trim.trim().is_empty() {
            return Err(ParseError::new("vfork arguments", args));
        }
        Ok(Self {
            result: super::fork::ForkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfork_display() {
        assert_eq!(Vfork::from_raw(0).to_string(), "vfork() = 0");
        assert_eq!(Vfork::from_raw(456).to_string(), "vfork() = 456");
        assert_eq!(Vfork::from_raw(-11).to_string(), "vfork() = -1 EAGAIN");
    }

    #[test]
    fn vfork_parse() {
        assert_eq!(Vfork::from_str("vfork() = 0").unwrap(), Vfork::from_raw(0));
        assert_eq!(
            Vfork::from_str("vfork() = -1 ENOMEM").unwrap(),
            Vfork::from_raw(-12)
        );
        assert!(Vfork::from_str("vfork(1) = 1").is_err());
        assert!(Vfork::from_str("fork() = 1").is_err());
    }

    #[test]
    fn vfork_round_trip() {
        for line in ["vfork() = 0", "vfork() = 123", "vfork() = -1 EAGAIN"] {
            assert_eq!(Vfork::from_str(line).unwrap().to_string(), line);
        }
    }
}
