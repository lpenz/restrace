// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`removexattr(2)`] system call.
//!
//! [`removexattr(2)`]: https://man7.org/linux/man-pages/man2/removexattr.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `removexattr(2)` system call.
///
/// ```text
/// int removexattr(const char *pathname, const char *name);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removexattr {
    /// Pathname of the file whose extended attribute is removed, without the
    /// terminating NUL byte.
    pub pathname: CString,
    /// Name of the extended attribute, without the terminating NUL byte.
    pub name: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Removexattr {
    /// Builds a `removexattr(2)` record from its arguments and raw return
    /// value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` or `name` contains an interior NUL byte, which
    /// cannot happen for strings read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>, N: AsRef<[u8]>>(pathname: P, name: N, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            name: CString::new(name.as_ref()).expect("name contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Removexattr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "removexattr({:?}, {:?}) = {}",
            self.pathname.to_string_lossy(),
            self.name.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `removexattr("/tmp/f", "user.a") = 0` or `... = -1 ENODATA`.
impl FromStr for Removexattr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("removexattr call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("removexattr(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("removexattr arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let name = parse::unquote(parts[1].trim(), "name")?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            name: CString::new(name).map_err(|_| ParseError::new("name", parts[1]))?,
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
        let call = Removexattr::from_raw("/tmp/f", "user.a", 0);
        assert_eq!(call.to_string(), "removexattr(\"/tmp/f\", \"user.a\") = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Removexattr::from_raw("/tmp/f", "user.a", -1);
        assert_eq!(
            call.to_string(),
            "removexattr(\"/tmp/f\", \"user.a\") = -1 EPERM"
        );
        assert_eq!(call.result, OkResult::Errno(Errno::EPERM));
    }

    #[test]
    fn parse_success() {
        let line = "removexattr(\"/tmp/f\", \"user.a\") = 0";
        let call = Removexattr::from_str(line).unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/f");
        assert_eq!(call.name.to_str().unwrap(), "user.a");
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "removexattr(\"/tmp/f\", \"user.a\") = -1 ENODATA";
        let call = Removexattr::from_str(line).unwrap();
        assert_eq!(call.result, OkResult::Errno(Errno(61)));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Removexattr::from_str("removexattr(\"/f\", \"n\") 0").is_err());
        assert!(Removexattr::from_str("removexattr(\"/f\") = 0").is_err());
        assert!(Removexattr::from_str("removexattr(\"/f\", \"n\", 1) = 0").is_err());
        assert!(Removexattr::from_str("removexattr(\"/f\", 0x1) = 0").is_err());
        assert!(Removexattr::from_str("fremovexattr(3, \"n\") = 0").is_err());
        assert!(Removexattr::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "removexattr(\"/tmp/f\", \"user.a\") = 0",
            "removexattr(\"/tmp/f\", \"user.a\") = -1 ENODATA",
            "removexattr(\"/a, b\", \"user.a, b\") = 0",
        ] {
            assert_eq!(Removexattr::from_str(line).unwrap().to_string(), line);
        }
    }
}
