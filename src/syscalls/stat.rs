// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`stat(2)`] system call.
//!
//! [`stat(2)`]: https://man7.org/linux/man-pages/man2/stat.2.html

use super::buffer::Buffer;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::statbuf::StatBuf;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `stat(2)` system call.
///
/// ```text
/// int stat(const char *restrict pathname, struct stat *restrict statbuf);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    /// Pathname of the file to stat, without the terminating NUL byte.
    pub pathname: CString,
    /// The `struct stat` the call filled in, or the raw address of the buffer
    /// when `strace` could not decode it, such as after a failure.
    pub buf: Buffer<StatBuf>,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Stat {
    /// Builds a `stat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(pathname: P, buf: Buffer<StatBuf>, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            buf,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stat({:?}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.buf,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `stat("path", {st_dev=..., ...}) = 0` or `stat("path", 0xADDR) = -1 ENOENT`.
impl FromStr for Stat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("stat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("stat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("stat arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            buf: super::buffer::parse(parts[1].trim())?,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::device::Device;
    use crate::syscalls::errno::Errno;
    use crate::syscalls::filemode::FileMode;

    fn buf() -> StatBuf {
        StatBuf {
            dev: Device::new(0, 0x1e),
            ino: 6997,
            mode: FileMode(0o100644),
            nlink: 1,
            uid: 0,
            gid: 0,
            blksize: 4096,
            blocks: 8,
            rdev: None,
            size: Some(8),
            atime: 1754766014,
            atime_nsec: 986788270,
            mtime: 1545480610,
            mtime_nsec: 541614343,
            ctime: 1754766014,
            ctime_nsec: 986788270,
        }
    }

    #[test]
    fn display_success() {
        let stat = Stat::from_raw("/etc/hostname", Buffer::Decoded(buf()), 0);
        assert_eq!(
            stat.to_string(),
            "stat(\"/etc/hostname\", {st_dev=makedev(0, 0x1e), st_ino=6997, \
             st_mode=S_IFREG|0644, st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, \
             st_blocks=8, st_size=8, st_atime=1754766014, st_atime_nsec=986788270, \
             st_mtime=1545480610, st_mtime_nsec=541614343, st_ctime=1754766014, \
             st_ctime_nsec=986788270}) = 0"
        );
    }

    #[test]
    fn display_failure() {
        let stat = Stat::from_raw("/nonexistent", Buffer::Address(0x7ffc1234), -2);
        assert_eq!(
            stat.to_string(),
            "stat(\"/nonexistent\", 0x7ffc1234) = -1 ENOENT"
        );
        assert_eq!(stat.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_success() {
        let stat = Stat::from_str(
            "stat(\"/etc/hostname\", {st_dev=makedev(0, 0x1e), st_ino=6997, \
             st_mode=S_IFREG|0644, st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, \
             st_blocks=8, st_size=8, st_atime=1754766014, st_atime_nsec=986788270, \
             st_mtime=1545480610, st_mtime_nsec=541614343, st_ctime=1754766014, \
             st_ctime_nsec=986788270}) = 0",
        )
        .unwrap();
        assert_eq!(stat.pathname.to_str().unwrap(), "/etc/hostname");
        assert_eq!(stat.buf, Buffer::Decoded(buf()));
        assert_eq!(stat.result, OkResult::Ok);
    }

    #[test]
    fn parse_failure() {
        let stat = Stat::from_str("stat(\"/nonexistent\", 0x7ffd684fe7b0) = -1 ENOENT").unwrap();
        assert_eq!(stat.buf, Buffer::Address(0x7ffd684fe7b0));
        assert_eq!(stat.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Stat::from_str("stat(\"/f\") = 0").is_err());
        assert!(Stat::from_str("stat(\"/f\", 0x1, 0x2) = 0").is_err());
        assert!(Stat::from_str("stat(/f, 0x1) = 0").is_err());
        assert!(Stat::from_str("lstat(\"/f\", 0x1) = 0").is_err());
        assert!(Stat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "stat(\"/nonexistent\", 0x7ffd684fe7b0) = -1 ENOENT",
            "stat(\"/f\", NULL) = -1 EACCES",
        ] {
            assert_eq!(Stat::from_str(line).unwrap().to_string(), line);
        }
        let with_buf = Stat {
            pathname: CString::new("/etc/hostname").unwrap(),
            buf: Buffer::Decoded(buf()),
            result: OkResult::Ok,
        };
        assert_eq!(Stat::from_str(&with_buf.to_string()).unwrap(), with_buf);
    }
}
