// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`mknod(2)`] system call.
//!
//! [`mknod(2)`]: https://man7.org/linux/man-pages/man2/mknod.2.html

use super::device::Device;
use super::filemode::FileMode;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `mknod(2)` system call.
///
/// ```text
/// int mknod(const char *pathname, mode_t mode, dev_t dev);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mknod {
    /// Pathname of the node to create, without the terminating NUL byte.
    pub pathname: CString,
    /// Node type and creation mode, the `mode` argument.
    pub mode: FileMode,
    /// Device number of a character or block device node, the `dev`
    /// argument.
    ///
    /// `strace` prints it only when the type in `mode` is `S_IFCHR` or
    /// `S_IFBLK`; for the other node types it is `None`.
    pub device: Option<Device>,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Mknod {
    /// Builds a `mknod(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        pathname: P,
        mode: FileMode,
        device: Option<Device>,
        raw_result: i64,
    ) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            device,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Mknod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mknod({:?}, {}",
            self.pathname.to_string_lossy(),
            self.mode
        )?;
        if let Some(device) = self.device {
            write!(f, ", {device}")?;
        }
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `mknod("path", S_IFIFO|0644) = 0` or
/// `mknod("path", S_IFCHR|0666, makedev(0x1, 0x3)) = 0`.
impl FromStr for Mknod {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("mknod call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("mknod(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if !(2..=3).contains(&parts.len()) {
            return Err(ParseError::new("mknod arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let mode = FileMode::from_str(parts[1].trim())?;
        let device = parts
            .get(2)
            .map(|part| Device::from_str(part.trim()))
            .transpose()?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            mode,
            device,
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
        let call = Mknod::from_raw("/tmp/fifo", FileMode(0o010644), None, 0);
        assert_eq!(call.to_string(), "mknod(\"/tmp/fifo\", S_IFIFO|0644) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_device_and_failure() {
        let call = Mknod::from_raw("/tmp/dev", FileMode(0o020666), Some(Device::new(1, 3)), -1);
        assert_eq!(
            call.to_string(),
            "mknod(\"/tmp/dev\", S_IFCHR|0666, makedev(0x1, 0x3)) = -1 EPERM"
        );
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Mknod::from_str("mknod(\"/tmp/fifo\", S_IFIFO|0644) = 0").unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/fifo");
        assert_eq!(call.mode, FileMode(0o010644));
        assert_eq!(call.device, None);
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), "mknod(\"/tmp/fifo\", S_IFIFO|0644) = 0");
    }

    #[test]
    fn parse_device() {
        let call =
            Mknod::from_str("mknod(\"/tmp/dev\", S_IFCHR|0666, makedev(0x1, 0x3)) = 0").unwrap();
        assert_eq!(call.mode, FileMode(0o020666));
        assert_eq!(call.device, Some(Device::new(1, 3)));
    }

    #[test]
    fn parse_failure() {
        let call = Mknod::from_str("mknod(\"/tmp/dev\", S_IFBLK|0600, makedev(8, 0)) = -1 EACCES")
            .unwrap();
        assert_eq!(call.result, OkResult::Errno(Errno::EACCES));
        assert_eq!(call.device, Some(Device::new(8, 0)));
    }

    #[test]
    fn parse_invalid() {
        assert!(Mknod::from_str("mknod(\"/tmp/f\", S_IFIFO|0644) 0").is_err());
        assert!(Mknod::from_str("mknod(/tmp/f, S_IFIFO|0644) = 0").is_err());
        assert!(Mknod::from_str("mknod(\"/tmp/f\") = 0").is_err());
        assert!(Mknod::from_str("mknod(\"/tmp/f\", S_IFIFO|0644, 1, 2) = 0").is_err());
        assert!(Mknod::from_str("mknodat(AT_FDCWD, \"/f\", S_IFIFO|0644) = 0").is_err());
        assert!(Mknod::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "mknod(\"/tmp/fifo\", S_IFIFO|0644) = 0",
            "mknod(\"/tmp/sock\", S_IFSOCK|0600) = -1 EACCES",
            "mknod(\"/tmp/dev\", S_IFCHR|0666, makedev(0x1, 0x3)) = 0",
            "mknod(\"/tmp/dev\", S_IFBLK|S_ISUID|0600, makedev(0x8, 0)) = -1 EPERM",
            "mknod(\"/a, b\", 0600) = -1 ENOENT",
        ] {
            assert_eq!(Mknod::from_str(line).unwrap().to_string(), line);
        }
    }
}
