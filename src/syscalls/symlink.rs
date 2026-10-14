// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`symlink(2)`] system call.
//!
//! [`symlink(2)`]: https://man7.org/linux/man-pages/man2/symlink.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `symlink(2)` system call.
///
/// ```text
/// int symlink(const char *target, const char *linkpath);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symlink {
    /// Contents of the symbolic link, without the terminating NUL byte.
    pub target: CString,
    /// Pathname of the symbolic link to create, without the terminating NUL
    /// byte.
    pub linkpath: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Symlink {
    /// Builds a `symlink(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if a pathname contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>, Q: AsRef<[u8]>>(
        target: P,
        linkpath: Q,
        raw_result: i64,
    ) -> Self {
        Self {
            target: CString::new(target.as_ref()).expect("pathname contains NUL byte"),
            linkpath: CString::new(linkpath.as_ref()).expect("pathname contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Symlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "symlink({:?}, {:?}) = {}",
            self.target.to_string_lossy(),
            self.linkpath.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `symlink("target", "path") = 0` or `... = -1 EEXIST`.
impl FromStr for Symlink {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("symlink call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("symlink(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("symlink arguments", args));
        }
        let target = parse::unquote(parts[0].trim(), "target")?;
        let linkpath = parse::unquote(parts[1].trim(), "linkpath")?;
        Ok(Self {
            target: CString::new(target).map_err(|_| ParseError::new("target", parts[0]))?,
            linkpath: CString::new(linkpath).map_err(|_| ParseError::new("linkpath", parts[1]))?,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let symlink = Symlink::from_raw("target", "/tmp/link", 0);
        assert_eq!(
            symlink.to_string(),
            "symlink(\"target\", \"/tmp/link\") = 0"
        );
        assert!(symlink.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let symlink = Symlink::from_raw("target", "/tmp/link", -17);
        assert_eq!(
            symlink.to_string(),
            "symlink(\"target\", \"/tmp/link\") = -1 EEXIST"
        );
    }

    #[test]
    fn parse_success() {
        let symlink = Symlink::from_str("symlink(\"target\", \"/tmp/link\") = 0").unwrap();
        assert_eq!(symlink, Symlink::from_raw("target", "/tmp/link", 0));
    }

    #[test]
    fn parse_failure() {
        let symlink = Symlink::from_str("symlink(\"target\", \"/tmp/link\") = -1 EACCES").unwrap();
        assert_eq!(symlink.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Symlink::from_str("symlink(\"t\", \"l\") 0").is_err());
        assert!(Symlink::from_str("symlink(\"t\") = 0").is_err());
        assert!(Symlink::from_str("symlink(t, l) = 0").is_err());
        assert!(Symlink::from_str("symlinkat(\"t\", AT_FDCWD, \"l\") = 0").is_err());
        assert!(Symlink::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "symlink(\"target\", \"/tmp/link\") = 0",
            "symlink(\"../a, b\", \"l\") = -1 EEXIST",
            "symlink(\"t\", \"quote\\\"here\") = -1 EACCES",
        ] {
            assert_eq!(Symlink::from_str(line).unwrap().to_string(), line);
        }
    }
}
