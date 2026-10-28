// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fremovexattr(2)`] system call.
//!
//! [`fremovexattr(2)`]: https://man7.org/linux/man-pages/man2/fremovexattr.2.html

use super::fd::Fd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fremovexattr(2)` system call.
///
/// ```text
/// int fremovexattr(int fd, const char *name);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fremovexattr {
    /// File descriptor of the file whose extended attribute is removed, with
    /// the path `strace` resolved for it when run with `-y`.
    pub fd: Fd,
    /// Name of the extended attribute, without the terminating NUL byte.
    pub name: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fremovexattr {
    /// Builds a `fremovexattr(2)` record from its arguments and raw return
    /// value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `name` contains an interior NUL byte, which cannot happen
    /// for strings read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<N: AsRef<[u8]>>(fd: i32, name: N, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            name: CString::new(name.as_ref()).expect("name contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fremovexattr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fremovexattr({}, {:?}) = {}",
            self.fd,
            self.name.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fremovexattr(3</tmp/f>, "user.a") = 0` or `... = -1 EBADF`.
impl FromStr for Fremovexattr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fremovexattr call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fremovexattr(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("fremovexattr arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let name = parse::unquote(parts[1].trim(), "name")?;
        Ok(Self {
            fd,
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
        let call = Fremovexattr::from_raw(3, "user.a", 0);
        assert_eq!(call.to_string(), "fremovexattr(3, \"user.a\") = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Fremovexattr::from_raw(3, "user.a", -9);
        assert_eq!(call.to_string(), "fremovexattr(3, \"user.a\") = -1 EBADF");
        assert_eq!(call.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_success() {
        let line = "fremovexattr(3</tmp/f>, \"user.a\") = 0";
        let call = Fremovexattr::from_str(line).unwrap();
        assert_eq!(call.fd.fd(), 3);
        assert_eq!(
            call.fd.path().map(|path| path.to_str().unwrap()),
            Some("/tmp/f")
        );
        assert_eq!(call.name.to_str().unwrap(), "user.a");
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "fremovexattr(3, \"user.a\") = -1 ENODATA";
        let call = Fremovexattr::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.result, OkResult::Errno(Errno(61)));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Fremovexattr::from_str("fremovexattr(3, \"n\") 0").is_err());
        assert!(Fremovexattr::from_str("fremovexattr(3) = 0").is_err());
        assert!(Fremovexattr::from_str("fremovexattr(3, \"n\", 1) = 0").is_err());
        assert!(Fremovexattr::from_str("fremovexattr(x, \"n\") = 0").is_err());
        assert!(Fremovexattr::from_str("fremovexattr(3, 0x1) = 0").is_err());
        assert!(Fremovexattr::from_str("removexattr(\"/f\", \"n\") = 0").is_err());
        assert!(Fremovexattr::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fremovexattr(3, \"user.a\") = 0",
            "fremovexattr(3</tmp/f>, \"user.a\") = 0",
            "fremovexattr(3, \"user.a\") = -1 ENODATA",
            "fremovexattr(3, \"user.a, b\") = 0",
        ] {
            assert_eq!(Fremovexattr::from_str(line).unwrap().to_string(), line);
        }
    }
}
