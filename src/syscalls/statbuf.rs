// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The decoded `struct stat` buffer of the `stat(2)` family.

use super::device::Device;
use super::filemode::FileMode;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Parses an unsigned decimal field.
fn parse_u64(input: &str, part: &'static str) -> Result<u64, ParseError> {
    input
        .trim()
        .parse::<u64>()
        .map_err(|_| ParseError::new(part, input))
}

/// The `struct stat` a `stat(2)`-family call filled in.
///
/// Strace prints it with `-v`, which the crate always passes, so all the
/// fields are shown. The `st_atime`/`st_mtime`/`st_ctime` comments that carry
/// a human-readable date are not stored: like the errno description, they are
/// derived from the value and dropped when parsing.
///
/// A `stat` buffer always has both `st_rdev` and `st_size`; strace prints
/// `st_rdev` for character and block devices and `st_size` for everything
/// else, so only the one it printed is present here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatBuf {
    /// Device the file resides on, the `st_dev` field.
    pub dev: Device,
    /// Inode number, the `st_ino` field.
    pub ino: u64,
    /// Symbolic file mode, the `st_mode` field.
    pub mode: FileMode,
    /// Number of hard links, the `st_nlink` field.
    pub nlink: u64,
    /// Owner user ID, the `st_uid` field.
    pub uid: u32,
    /// Owner group ID, the `st_gid` field.
    pub gid: u32,
    /// Preferred block size for filesystem I/O, the `st_blksize` field.
    pub blksize: u64,
    /// Number of 512-byte blocks allocated, the `st_blocks` field.
    pub blocks: u64,
    /// Device ID for character and block devices, the `st_rdev` field.
    pub rdev: Option<Device>,
    /// Total size in bytes, the `st_size` field, for everything that is not
    /// a character or block device.
    pub size: Option<u64>,
    /// Last access time, in seconds, the `st_atime` field.
    pub atime: i64,
    /// Nanoseconds of the last access time, the `st_atime_nsec` field.
    pub atime_nsec: u64,
    /// Last modification time, in seconds, the `st_mtime` field.
    pub mtime: i64,
    /// Nanoseconds of the last modification time, the `st_mtime_nsec` field.
    pub mtime_nsec: u64,
    /// Last status change time, in seconds, the `st_ctime` field.
    pub ctime: i64,
    /// Nanoseconds of the last status change time, the `st_ctime_nsec` field.
    pub ctime_nsec: u64,
}

impl fmt::Display for StatBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{st_dev={}, st_ino={}, st_mode={}, st_nlink={}, st_uid={}, st_gid={}, \
             st_blksize={}, st_blocks={}",
            self.dev,
            self.ino,
            self.mode,
            self.nlink,
            self.uid,
            self.gid,
            self.blksize,
            self.blocks
        )?;
        match (self.rdev, self.size) {
            (Some(rdev), _) => write!(f, ", st_rdev={rdev}")?,
            (None, Some(size)) => write!(f, ", st_size={size}")?,
            (None, None) => return Err(fmt::Error),
        }
        write!(
            f,
            ", st_atime={}, st_atime_nsec={}, st_mtime={}, st_mtime_nsec={}, \
             st_ctime={}, st_ctime_nsec={}}}",
            self.atime, self.atime_nsec, self.mtime, self.mtime_nsec, self.ctime, self.ctime_nsec
        )
    }
}

/// Parses a timestamp field, dropping the trailing ` /* date */` comment that
/// strace appends.
fn parse_time(input: &str, part: &'static str) -> Result<i64, ParseError> {
    parse::strip_comment(input.trim())
        .parse::<i64>()
        .map_err(|_| ParseError::new(part, input))
}

/// Parses a `struct stat` as [`Display`](fmt::Display) writes it:
/// `{st_dev=makedev(0, 0x1e), st_ino=1, ...}`.
impl FromStr for StatBuf {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("stat buffer", s);
        let body = s
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .ok_or_else(error)?;
        let mut dev = None;
        let mut ino = None;
        let mut mode = None;
        let mut nlink = None;
        let mut uid = None;
        let mut gid = None;
        let mut blksize = None;
        let mut blocks = None;
        let mut rdev = None;
        let mut size = None;
        let mut atime = None;
        let mut atime_nsec = None;
        let mut mtime = None;
        let mut mtime_nsec = None;
        let mut ctime = None;
        let mut ctime_nsec = None;
        for field in parse::split_args_nested(body) {
            let (key, value) = field.split_once('=').ok_or_else(error)?;
            match key.trim() {
                "st_dev" => dev = Some(Device::from_str(value)?),
                "st_ino" => ino = Some(parse_u64(value, "st_ino")?),
                "st_mode" => mode = Some(FileMode::from_str(value)?),
                "st_nlink" => nlink = Some(parse_u64(value, "st_nlink")?),
                "st_uid" => {
                    uid = Some(
                        value
                            .trim()
                            .parse::<u32>()
                            .map_err(|_| ParseError::new("st_uid", value))?,
                    );
                }
                "st_gid" => {
                    gid = Some(
                        value
                            .trim()
                            .parse::<u32>()
                            .map_err(|_| ParseError::new("st_gid", value))?,
                    );
                }
                "st_blksize" => blksize = Some(parse_u64(value, "st_blksize")?),
                "st_blocks" => blocks = Some(parse_u64(value, "st_blocks")?),
                "st_rdev" => rdev = Some(Device::from_str(value)?),
                "st_size" => size = Some(parse_u64(value, "st_size")?),
                "st_atime" => atime = Some(parse_time(value, "st_atime")?),
                "st_atime_nsec" => atime_nsec = Some(parse_u64(value, "st_atime_nsec")?),
                "st_mtime" => mtime = Some(parse_time(value, "st_mtime")?),
                "st_mtime_nsec" => mtime_nsec = Some(parse_u64(value, "st_mtime_nsec")?),
                "st_ctime" => ctime = Some(parse_time(value, "st_ctime")?),
                "st_ctime_nsec" => ctime_nsec = Some(parse_u64(value, "st_ctime_nsec")?),
                _ => return Err(ParseError::new("stat field", field)),
            }
        }
        Ok(Self {
            dev: dev.ok_or_else(error)?,
            ino: ino.ok_or_else(error)?,
            mode: mode.ok_or_else(error)?,
            nlink: nlink.ok_or_else(error)?,
            uid: uid.ok_or_else(error)?,
            gid: gid.ok_or_else(error)?,
            blksize: blksize.ok_or_else(error)?,
            blocks: blocks.ok_or_else(error)?,
            rdev,
            size,
            atime: atime.ok_or_else(error)?,
            atime_nsec: atime_nsec.ok_or_else(error)?,
            mtime: mtime.ok_or_else(error)?,
            mtime_nsec: mtime_nsec.ok_or_else(error)?,
            ctime: ctime.ok_or_else(error)?,
            ctime_nsec: ctime_nsec.ok_or_else(error)?,
        })
    }
}

impl super::buffer::BufferValue for StatBuf {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        Self::from_str(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGULAR: &str = "{st_dev=makedev(0, 0x1e), st_ino=6997, st_mode=S_IFREG|0644, \
        st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, st_blocks=8, st_size=8, \
        st_atime=1754766014, st_atime_nsec=986788270, st_mtime=1545480610, \
        st_mtime_nsec=541614343, st_ctime=1754766014, st_ctime_nsec=986788270}";

    fn regular() -> StatBuf {
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
    fn display_size() {
        assert_eq!(regular().to_string(), REGULAR);
    }

    #[test]
    fn display_rdev() {
        let buf = StatBuf {
            mode: FileMode(0o20666),
            rdev: Some(Device::new(1, 3)),
            size: None,
            ..regular()
        };
        assert!(buf.to_string().contains(", st_rdev=makedev(0x1, 0x3), "));
        assert!(!buf.to_string().contains("st_size"));
    }

    #[test]
    fn parse_with_comment() {
        let line = "{st_dev=makedev(0, 0x1e), st_ino=6997, st_mode=S_IFREG|0644, \
            st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, st_blocks=8, st_size=8, \
            st_atime=1754766014 /* 2025-08-09T20:00:14.986788270+0100 */, \
            st_atime_nsec=986788270, \
            st_mtime=1545480610 /* 2018-12-22T12:10:10.541614343+0000 */, \
            st_mtime_nsec=541614343, \
            st_ctime=1754766014 /* 2025-08-09T20:00:14.986788270+0100 */, \
            st_ctime_nsec=986788270}";
        assert_eq!(StatBuf::from_str(line), Ok(regular()));
    }

    #[test]
    fn parse_rdev() {
        let line = "{st_dev=makedev(0, 0x7), st_ino=4, st_mode=S_IFCHR|0666, st_nlink=1, \
            st_uid=0, st_gid=0, st_blksize=4096, st_blocks=0, st_rdev=makedev(0x1, 0x3), \
            st_atime=1, st_atime_nsec=2, st_mtime=3, st_mtime_nsec=4, st_ctime=5, \
            st_ctime_nsec=6}";
        let buf = StatBuf::from_str(line).unwrap();
        assert_eq!(buf.rdev, Some(Device::new(1, 3)));
        assert_eq!(buf.size, None);
        assert_eq!(buf.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(StatBuf::from_str("{}").is_err());
        assert!(StatBuf::from_str("st_dev=makedev(0, 0)").is_err());
        assert!(StatBuf::from_str("{st_bogus=1}").is_err());
        assert!(StatBuf::from_str(&REGULAR.replace("st_ino=6997, ", "")).is_err());
    }

    #[test]
    fn round_trip() {
        assert_eq!(StatBuf::from_str(REGULAR).unwrap(), regular());
        assert_eq!(regular().to_string(), REGULAR);
    }
}
