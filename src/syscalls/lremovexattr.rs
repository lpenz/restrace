// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`lremovexattr(2)`] system call.
//!
//! [`lremovexattr(2)`]: https://man7.org/linux/man-pages/man2/lremovexattr.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `lremovexattr(2)` system call.
///
/// ```text
/// int lremovexattr(const char *pathname, const char *name);
/// ```
///
/// Unlike [`removexattr(2)`](super::removexattr::Removexattr), the call does
/// not follow a symbolic link in `pathname`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lremovexattr {
    /// Pathname of the file whose extended attribute is removed, without the
    /// terminating NUL byte.
    pub pathname: CString,
    /// Name of the extended attribute, without the terminating NUL byte.
    pub name: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Lremovexattr {
    /// Builds a `lremovexattr(2)` record from its arguments and raw return
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

impl fmt::Display for Lremovexattr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lremovexattr({:?}, {:?}) = {}",
            self.pathname.to_string_lossy(),
            self.name.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `lremovexattr("/tmp/f", "user.a") = 0` or `... = -1 ENODATA`.
impl FromStr for Lremovexattr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("lremovexattr call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("lremovexattr(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("lremovexattr arguments", args));
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
        let call = Lremovexattr::from_raw("/tmp/f", "user.a", 0);
        assert_eq!(call.to_string(), "lremovexattr(\"/tmp/f\", \"user.a\") = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Lremovexattr::from_raw("/tmp/f", "user.a", -1);
        assert_eq!(
            call.to_string(),
            "lremovexattr(\"/tmp/f\", \"user.a\") = -1 EPERM"
        );
        assert_eq!(call.result, OkResult::Errno(Errno::EPERM));
    }

    #[test]
    fn parse_success() {
        let line = "lremovexattr(\"/tmp/f\", \"user.a\") = 0";
        let call = Lremovexattr::from_str(line).unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/f");
        assert_eq!(call.name.to_str().unwrap(), "user.a");
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "lremovexattr(\"/tmp/f\", \"user.a\") = -1 ENODATA";
        let call = Lremovexattr::from_str(line).unwrap();
        assert_eq!(call.result, OkResult::Errno(Errno(61)));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Lremovexattr::from_str("lremovexattr(\"/f\", \"n\") 0").is_err());
        assert!(Lremovexattr::from_str("lremovexattr(\"/f\") = 0").is_err());
        assert!(Lremovexattr::from_str("lremovexattr(\"/f\", \"n\", 1) = 0").is_err());
        assert!(Lremovexattr::from_str("lremovexattr(\"/f\", 0x1) = 0").is_err());
        assert!(Lremovexattr::from_str("removexattr(\"/f\", \"n\") = 0").is_err());
        assert!(Lremovexattr::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "lremovexattr(\"/tmp/f\", \"user.a\") = 0",
            "lremovexattr(\"/tmp/f\", \"user.a\") = -1 ENODATA",
            "lremovexattr(\"/a, b\", \"user.a, b\") = 0",
        ] {
            assert_eq!(Lremovexattr::from_str(line).unwrap().to_string(), line);
        }
    }
}
