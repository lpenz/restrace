// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The exit of a traced process.
//!
//! Unlike the system calls, a process exit is not written by `strace` as a
//! call with arguments and a return value: it is a standalone `+++ ... +++`
//! line that reports how the process terminated. [`Exit`] records either a
//! normal exit code or the signal that killed the process.

use crate::syscalls::ParseError;
use std::fmt;
use std::str::FromStr;

/// How a traced process terminated, as reported by `strace`.
///
/// ```text
/// 464435 1791839404.885951 +++ killed by SIGKILL +++
/// 464434 1791839404.886503 +++ exited with 3 +++
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// The process called `exit`/`exit_group` and terminated with this status
    /// code, printed as `+++ exited with <code> +++`.
    Code(i32),
    /// The process was killed by the given signal, printed as
    /// `+++ killed by <name> [(core dumped)] +++`.
    Signal {
        /// The name of the signal, as `strace` prints it, e.g. `SIGKILL`.
        name: String,
        /// Whether the signal dumped a core file.
        core_dumped: bool,
    },
}

impl fmt::Display for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code(code) => write!(f, "+++ exited with {code} +++"),
            Self::Signal { name, core_dumped } => write!(
                f,
                "+++ killed by {name} {}+++",
                if *core_dumped { "(core dumped) " } else { "" }
            ),
        }
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it: `+++ exited with
/// <code> +++` or `+++ killed by <name> [(core dumped)] +++`.
impl FromStr for Exit {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("exit", s);
        let body = s
            .strip_prefix("+++ ")
            .and_then(|s| s.strip_suffix(" +++"))
            .ok_or_else(error)?;
        if let Some(code) = body.strip_prefix("exited with ") {
            return code.parse::<i32>().map(Self::Code).map_err(|_| error());
        }
        if let Some(rest) = body.strip_prefix("killed by ") {
            if let Some(name) = rest.strip_suffix(" (core dumped)") {
                if name.is_empty() {
                    return Err(error());
                }
                return Ok(Self::Signal {
                    name: name.to_string(),
                    core_dumped: true,
                });
            }
            if rest.is_empty() || rest.contains(char::is_whitespace) {
                return Err(error());
            }
            return Ok(Self::Signal {
                name: rest.to_string(),
                core_dumped: false,
            });
        }
        Err(error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_code() {
        assert_eq!(Exit::Code(3).to_string(), "+++ exited with 3 +++");
        assert_eq!(Exit::Code(0).to_string(), "+++ exited with 0 +++");
        assert_eq!(Exit::Code(-1).to_string(), "+++ exited with -1 +++");
    }

    #[test]
    fn display_signal() {
        assert_eq!(
            Exit::Signal {
                name: "SIGKILL".to_string(),
                core_dumped: false,
            }
            .to_string(),
            "+++ killed by SIGKILL +++"
        );
        assert_eq!(
            Exit::Signal {
                name: "SIGSEGV".to_string(),
                core_dumped: true,
            }
            .to_string(),
            "+++ killed by SIGSEGV (core dumped) +++"
        );
    }

    #[test]
    fn parse_code() {
        assert_eq!(
            Exit::from_str("+++ exited with 3 +++").unwrap(),
            Exit::Code(3)
        );
        assert_eq!(
            Exit::from_str("+++ exited with 0 +++").unwrap(),
            Exit::Code(0)
        );
    }

    #[test]
    fn parse_signal() {
        assert_eq!(
            Exit::from_str("+++ killed by SIGKILL +++").unwrap(),
            Exit::Signal {
                name: "SIGKILL".to_string(),
                core_dumped: false,
            }
        );
        assert_eq!(
            Exit::from_str("+++ killed by SIGSEGV (core dumped) +++").unwrap(),
            Exit::Signal {
                name: "SIGSEGV".to_string(),
                core_dumped: true,
            }
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(Exit::from_str("").is_err());
        assert!(Exit::from_str("+++ exited with +++").is_err());
        assert!(Exit::from_str("+++ exited with x +++").is_err());
        assert!(Exit::from_str("+++ killed by  +++").is_err());
        assert!(Exit::from_str("+++ killed by  (core dumped) +++").is_err());
        assert!(Exit::from_str("+++ killed by SIG KILL +++").is_err());
        assert!(Exit::from_str("+++ something else +++").is_err());
        assert!(Exit::from_str("exited with 3").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "+++ exited with 0 +++",
            "+++ exited with 3 +++",
            "+++ exited with -1 +++",
            "+++ killed by SIGKILL +++",
            "+++ killed by SIGTERM +++",
            "+++ killed by SIGSEGV (core dumped) +++",
        ] {
            assert_eq!(Exit::from_str(line).unwrap().to_string(), line);
        }
    }
}
