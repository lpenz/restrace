// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`execve(2)`] system call.
//!
//! [`execve(2)`]: https://man7.org/linux/man-pages/man2/execve.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Writes a string array as strace does: `["first", "second"]`.
pub(crate) fn write_str_array(f: &mut fmt::Formatter<'_>, values: &[CString]) -> fmt::Result {
    f.write_str("[")?;
    for (i, value) in values.iter().enumerate() {
        if i != 0 {
            f.write_str(", ")?;
        }
        write!(f, "{:?}", value.to_string_lossy())?;
    }
    f.write_str("]")
}

/// Parses a string array as [`write_str_array`] writes it: a bracket-delimited,
/// comma-separated list of double-quoted strings.
pub(crate) fn parse_str_array(input: &str, part: &'static str) -> Result<Vec<CString>, ParseError> {
    let error = || ParseError::new(part, input);
    let body = input
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(error)?;
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for item in parse::split_args(body) {
        let value = parse::unquote(item.trim(), part)?;
        values.push(CString::new(value).map_err(|_| error())?);
    }
    Ok(values)
}

/// Arguments and return value of the `execve(2)` system call.
///
/// ```text
/// int execve(const char *pathname, char *const argv[], char *const envp[]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execve {
    /// Pathname of the program to run, without the terminating NUL byte.
    pub pathname: CString,
    /// The argument vector, the `argv` argument.
    pub argv: Vec<CString>,
    /// The environment, the `envp` argument.
    pub envp: Vec<CString>,
    /// Return value: zero on success, or the errno of the failure.
    ///
    /// On success the calling process is replaced and execve does not return;
    /// strace still reports `= 0`.
    pub result: OkResult,
}

impl Execve {
    /// Builds an `execve(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` or any array entry contains an interior NUL byte,
    /// which cannot happen for strings read from a tracee.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        pathname: P,
        argv: Vec<CString>,
        envp: Vec<CString>,
        raw_result: i64,
    ) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            argv,
            envp,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Execve {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "execve({:?}, ", self.pathname.to_string_lossy())?;
        write_str_array(f, &self.argv)?;
        f.write_str(", ")?;
        write_str_array(f, &self.envp)?;
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `execve("path", ["argv"], ["env"]) = 0` or `... = -1 ENOENT`.
impl FromStr for Execve {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("execve call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("execve(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("execve arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let argv = parse_str_array(parts[1], "argv")?;
        let envp = parse_str_array(parts[2], "envp")?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            argv,
            envp,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn display_success() {
        let execve = Execve::from_raw(
            "/bin/echo",
            vec![c("echo"), c("hello")],
            vec![c("PATH=/bin"), c("HOME=/root")],
            0,
        );
        assert_eq!(
            execve.to_string(),
            "execve(\"/bin/echo\", [\"echo\", \"hello\"], [\"PATH=/bin\", \"HOME=/root\"]) = 0"
        );
        assert!(execve.result.is_ok());
    }

    #[test]
    fn display_empty_arrays() {
        let execve = Execve::from_raw("/bin/true", vec![], vec![], 0);
        assert_eq!(execve.to_string(), "execve(\"/bin/true\", [], []) = 0");
    }

    #[test]
    fn parse_success() {
        let execve =
            Execve::from_str("execve(\"/bin/echo\", [\"echo\", \"hi\"], [\"TERM=xterm\"]) = 0")
                .unwrap();
        assert_eq!(
            execve,
            Execve::from_raw(
                "/bin/echo",
                vec![c("echo"), c("hi")],
                vec![c("TERM=xterm")],
                0
            )
        );
        assert_eq!(execve.argv[1].to_str().unwrap(), "hi");
    }

    #[test]
    fn parse_failure() {
        let execve =
            Execve::from_str("execve(\"/nonexistent\", [\"nonexistent\"], []) = -1 ENOENT")
                .unwrap();
        assert_eq!(execve.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Execve::from_str("execve(\"/bin/echo\") = 0").is_err());
        assert!(Execve::from_str("execve(\"/bin/echo\", [\"echo\"]) = 0").is_err());
        assert!(Execve::from_str("execve(\"/bin/echo\", echo, []) = 0").is_err());
        assert!(Execve::from_str("execve(\"/bin/echo\", [\"echo\"], 0x1000) = 0").is_err());
        assert!(Execve::from_str("execveat(AT_FDCWD, \"p\", [], [], 0) = 0").is_err());
        assert!(Execve::from_str("execve(\"/bin/echo\", [\"echo\"], []) 0").is_err());
        assert!(Execve::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "execve(\"/bin/echo\", [\"echo\", \"hello\"], [\"PATH=/bin\"]) = 0",
            "execve(\"/bin/true\", [], []) = 0",
            "execve(\"/nonexistent\", [\"arg\"], []) = -1 ENOENT",
            "execve(\"/bin/sh\", [\"sh\", \"-c\", \"echo , hi\"], []) = 0",
            "execve(\"/bin/echo\", [\"quote\\\"here\"], [\"A=B\", \"C=D\"]) = -1 EACCES",
        ] {
            assert_eq!(Execve::from_str(line).unwrap().to_string(), line);
        }
    }
}
