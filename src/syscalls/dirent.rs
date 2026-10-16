// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Directory entries decoded by the `getdents(2)` family of system calls.
//!
//! [`getdents(2)`]: https://man7.org/linux/man-pages/man2/getdents.2.html

use super::buffer::BufferValue;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// The `d_type` field of a directory entry.
///
/// The values are the `DT_*` constants of `dirent.h`. `strace` prints a type
/// that has no constant as `DT_???`, which [`DirentType::UNRECOGNIZED`]
/// represents; the numeric value is not recoverable from that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct DirentType(pub u8);

impl DirentType {
    /// Unknown file type (`DT_UNKNOWN`).
    pub const UNKNOWN: Self = Self(0);
    /// Named pipe (`DT_FIFO`).
    pub const FIFO: Self = Self(1);
    /// Character device (`DT_CHR`).
    pub const CHR: Self = Self(2);
    /// Directory (`DT_DIR`).
    pub const DIR: Self = Self(4);
    /// Block device (`DT_BLK`).
    pub const BLK: Self = Self(6);
    /// Regular file (`DT_REG`).
    pub const REG: Self = Self(8);
    /// Symbolic link (`DT_LNK`).
    pub const LNK: Self = Self(10);
    /// Socket (`DT_SOCK`).
    pub const SOCK: Self = Self(12);
    /// Whiteout (`DT_WHT`).
    pub const WHT: Self = Self(14);
    /// A type outside the known set, which `strace` prints as `DT_???`.
    pub const UNRECOGNIZED: Self = Self(0xff);
}

/// Named dirent types, in [`Display`](fmt::Display) order.
const NAMES: &[(u8, &str)] = &[
    (0, "DT_UNKNOWN"),
    (1, "DT_FIFO"),
    (2, "DT_CHR"),
    (4, "DT_DIR"),
    (6, "DT_BLK"),
    (8, "DT_REG"),
    (10, "DT_LNK"),
    (12, "DT_SOCK"),
    (14, "DT_WHT"),
];

impl fmt::Display for DirentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match NAMES.iter().find(|&&(value, _)| value == self.0) {
            Some(&(_, name)) => f.write_str(name),
            None => f.write_str("DT_???"),
        }
    }
}

/// Parses a type as [`Display`](fmt::Display) writes it: the `DT_*` name, or
/// `DT_???` for an unrecognized type.
impl FromStr for DirentType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s == "DT_???" {
            return Ok(Self::UNRECOGNIZED);
        }
        NAMES
            .iter()
            .find(|&&(_, name)| name == s)
            .map(|&(value, _)| Self(value))
            .ok_or_else(|| ParseError::new("dirent type", s))
    }
}

/// A directory entry as read by [`getdents(2)`](super::getdents::Getdents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirent {
    /// Inode number (`d_ino`).
    pub ino: u64,
    /// Offset of the next entry (`d_off`).
    pub off: u64,
    /// Length of this record (`d_reclen`).
    pub reclen: u64,
    /// Entry name (`d_name`), without the terminating NUL byte.
    pub name: CString,
    /// Entry type (`d_type`).
    pub dtype: DirentType,
}

impl fmt::Display for Dirent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{d_ino={}, d_off={}, d_reclen={}, d_name={:?}, d_type={}}}",
            self.ino,
            self.off,
            self.reclen,
            self.name.to_string_lossy(),
            self.dtype
        )
    }
}

impl FromStr for Dirent {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let (ino, off, reclen, name, dtype) = parse_fields(s)?;
        Ok(Self {
            ino,
            off,
            reclen,
            name,
            dtype,
        })
    }
}

/// A directory entry as read by
/// [`getdents64(2)`](super::getdents64::Getdents64).
///
/// It carries the same fields as [`Dirent`]; the two differ in the order
/// `strace` prints them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirent64 {
    /// Inode number (`d_ino`).
    pub ino: u64,
    /// Offset of the next entry (`d_off`).
    pub off: u64,
    /// Length of this record (`d_reclen`).
    pub reclen: u64,
    /// Entry type (`d_type`).
    pub dtype: DirentType,
    /// Entry name (`d_name`), without the terminating NUL byte.
    pub name: CString,
}

impl fmt::Display for Dirent64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{d_ino={}, d_off={}, d_reclen={}, d_type={}, d_name={:?}}}",
            self.ino,
            self.off,
            self.reclen,
            self.dtype,
            self.name.to_string_lossy()
        )
    }
}

impl FromStr for Dirent64 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let (ino, off, reclen, name, dtype) = parse_fields(s)?;
        Ok(Self {
            ino,
            off,
            reclen,
            dtype,
            name,
        })
    }
}

/// Parses the common fields of a `{...}` directory entry, in any order.
fn parse_fields(s: &str) -> Result<(u64, u64, u64, CString, DirentType), ParseError> {
    let s = s.trim();
    let error = || ParseError::new("dirent", s);
    let body = s
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or_else(error)?;
    let (mut ino, mut off, mut reclen, mut name, mut dtype) = (None, None, None, None, None);
    for field in parse::split_args_nested(body) {
        let (key, value) = field.trim().split_once('=').ok_or_else(error)?;
        let value = value.trim();
        match key {
            "d_ino" => ino = Some(value.parse::<u64>().map_err(|_| error())?),
            "d_off" => off = Some(value.parse::<u64>().map_err(|_| error())?),
            "d_reclen" => reclen = Some(value.parse::<u64>().map_err(|_| error())?),
            "d_name" => {
                let decoded = parse::unquote(value, "dirent name")?;
                name = Some(CString::new(decoded).map_err(|_| error())?);
            }
            "d_type" => dtype = Some(DirentType::from_str(value)?),
            _ => return Err(error()),
        }
    }
    Ok((
        ino.ok_or_else(error)?,
        off.ok_or_else(error)?,
        reclen.ok_or_else(error)?,
        name.ok_or_else(error)?,
        dtype.ok_or_else(error)?,
    ))
}

/// The decoded entry list of a `getdents(2)`-family buffer argument.
///
/// A plain [`Vec`] cannot implement [`Display`](fmt::Display), so the decoded
/// list is wrapped here to be usable as a [`Buffer`](super::buffer::Buffer)
/// value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirentList<T>(pub Vec<T>);

impl<T> DirentList<T> {
    /// Returns the entries as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Returns true if there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T: fmt::Display> fmt::Display for DirentList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        let mut separator = "";
        for entry in &self.0 {
            write!(f, "{separator}{entry}")?;
            separator = ", ";
        }
        f.write_str("]")
    }
}

/// Parses a `[...]` entry list where each entry is parsed by `T`.
fn decode_list<T: FromStr<Err = ParseError>>(input: &str) -> Result<Vec<T>, ParseError> {
    let input = input.trim();
    let body = input
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| ParseError::new("dirent list", input))?;
    let body = body.trim();
    if body.is_empty() {
        return Ok(Vec::new());
    }
    parse::split_args_nested(body)
        .into_iter()
        .map(|entry| entry.trim().parse::<T>())
        .collect()
}

impl BufferValue for DirentList<Dirent> {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        decode_list(input).map(Self)
    }
}

impl BufferValue for DirentList<Dirent64> {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        decode_list(input).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirent() -> Dirent {
        Dirent {
            ino: 54,
            off: 1,
            reclen: 24,
            name: CString::new(".").unwrap(),
            dtype: DirentType::DIR,
        }
    }

    fn dirent64() -> Dirent64 {
        Dirent64 {
            ino: 54,
            off: 1,
            reclen: 24,
            dtype: DirentType::REG,
            name: CString::new("hostname").unwrap(),
        }
    }

    #[test]
    fn display_dirent() {
        assert_eq!(
            dirent().to_string(),
            "{d_ino=54, d_off=1, d_reclen=24, d_name=\".\", d_type=DT_DIR}"
        );
    }

    #[test]
    fn display_dirent64() {
        assert_eq!(
            dirent64().to_string(),
            "{d_ino=54, d_off=1, d_reclen=24, d_type=DT_REG, d_name=\"hostname\"}"
        );
    }

    #[test]
    fn display_type_unknown() {
        assert_eq!(DirentType::UNRECOGNIZED.to_string(), "DT_???");
        assert_eq!(DirentType(3).to_string(), "DT_???");
        assert_eq!(DirentType::UNKNOWN.to_string(), "DT_UNKNOWN");
    }

    #[test]
    fn display_list() {
        let list = DirentList(vec![dirent()]);
        assert_eq!(
            list.to_string(),
            "[{d_ino=54, d_off=1, d_reclen=24, d_name=\".\", d_type=DT_DIR}]"
        );
        assert_eq!(DirentList::<Dirent>(Vec::new()).to_string(), "[]");
        assert_eq!(list.len(), 1);
        assert!(!list.is_empty());
        assert_eq!(list.as_slice(), &[dirent()]);
    }

    #[test]
    fn parse_dirent() {
        let dirent =
            Dirent::from_str("{d_ino=54, d_off=1, d_reclen=24, d_name=\".\", d_type=DT_DIR}")
                .unwrap();
        assert_eq!(dirent, self::dirent());
    }

    #[test]
    fn parse_dirent64() {
        let dirent = Dirent64::from_str(
            "{d_ino=54, d_off=1, d_reclen=24, d_type=DT_REG, d_name=\"hostname\"}",
        )
        .unwrap();
        assert_eq!(dirent, dirent64());
    }

    #[test]
    fn parse_list() {
        let list = DirentList::<Dirent>::decode_buffer("[]").unwrap();
        assert!(list.is_empty());
        let list = DirentList::<Dirent>::decode_buffer(
            "[{d_ino=54, d_off=1, d_reclen=24, d_name=\".\", d_type=DT_DIR}]",
        )
        .unwrap();
        assert_eq!(list, DirentList(vec![dirent()]));
    }

    #[test]
    fn parse_invalid() {
        assert!(Dirent::from_str("d_ino=54").is_err());
        assert!(Dirent::from_str("{d_ino=54, d_off=1, d_reclen=24, d_type=DT_BOGUS}").is_err());
        assert!(Dirent::from_str("{d_ino=54, d_off=1, d_reclen=24, d_name=\".\"}").is_err());
        assert!(
            Dirent::from_str("{d_ino=x, d_off=1, d_reclen=24, d_name=\".\", d_type=DT_DIR}")
                .is_err()
        );
        assert!(DirentType::from_str("DT_BOGUS").is_err());
        assert!(DirentList::<Dirent>::decode_buffer("[{}").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "{d_ino=54, d_off=1, d_reclen=24, d_name=\".\", d_type=DT_DIR}",
            "{d_ino=1234, d_off=2147483647, d_reclen=40, d_name=\"a b\\\"c\", d_type=DT_LNK}",
            "{d_ino=1, d_off=0, d_reclen=24, d_name=\"x\", d_type=DT_???}",
        ] {
            assert_eq!(Dirent::from_str(line).unwrap().to_string(), line);
        }
        for line in [
            "{d_ino=54, d_off=1, d_reclen=24, d_type=DT_REG, d_name=\"hostname\"}",
            "{d_ino=1, d_off=0, d_reclen=24, d_type=DT_???, d_name=\"x\"}",
        ] {
            assert_eq!(Dirent64::from_str(line).unwrap().to_string(), line);
        }
        let list = DirentList(vec![
            dirent(),
            Dirent {
                ino: 2,
                off: 2,
                reclen: 32,
                name: CString::new("hostname").unwrap(),
                dtype: DirentType::REG,
            },
        ]);
        let rendered = list.to_string();
        assert_eq!(
            DirentList::<Dirent>::decode_buffer(&rendered).unwrap(),
            list
        );
    }
}
