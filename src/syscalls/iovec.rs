// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Vectored buffers decoded by the `readv(2)` family of system calls.
//!
//! [`readv(2)`]: https://man7.org/linux/man-pages/man2/readv.2.html

use super::buffer::{self, Buffer, BufferValue};
use super::bytes::Bytes;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// One `struct iovec` entry of a `readv(2)`-family iov array.
///
/// ```text
/// struct iovec {
///     void  *iov_base;
///     size_t iov_len;
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iovec {
    /// Base of the buffer, holding the bytes `strace` decoded, or the raw
    /// address it shows when it could not decode them, such as after a
    /// failure.
    pub base: Buffer<Bytes>,
    /// Length of the buffer (`iov_len`).
    pub len: u64,
}

impl fmt::Display for Iovec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{iov_base={}, iov_len={}}}", self.base, self.len)
    }
}

/// Parses an entry as [`Display`](fmt::Display) writes it:
/// `{iov_base="data", iov_len=16}` or `{iov_base=0xADDR, iov_len=16}`.
impl FromStr for Iovec {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("iovec", s);
        let body = s
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .ok_or_else(error)?;
        let (mut base, mut len) = (None, None);
        for field in parse::split_args_nested(body) {
            let (key, value) = field.trim().split_once('=').ok_or_else(error)?;
            let value = value.trim();
            match key {
                "iov_base" => base = Some(buffer::parse::<Bytes>(value)?),
                "iov_len" => len = Some(value.parse::<u64>().map_err(|_| error())?),
                _ => return Err(error()),
            }
        }
        Ok(Self {
            base: base.ok_or_else(error)?,
            len: len.ok_or_else(error)?,
        })
    }
}

/// The decoded iov array argument of a `readv(2)`-family system call.
///
/// A plain [`Vec`] cannot implement [`Display`](fmt::Display), so the decoded
/// list is wrapped here to be usable as a [`Buffer`] value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IovecList(pub Vec<Iovec>);

impl IovecList {
    /// Returns the entries as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Iovec] {
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

impl fmt::Display for IovecList {
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

impl BufferValue for IovecList {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        let input = input.trim();
        let body = input
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| ParseError::new("iovec list", input))?;
        let body = body.trim();
        if body.is_empty() {
            return Ok(Self(Vec::new()));
        }
        let entries = parse::split_args_nested(body)
            .into_iter()
            .map(|entry| entry.trim().parse::<Iovec>())
            .collect::<Result<_, _>>()?;
        Ok(Self(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(value: &[u8]) -> Buffer<Bytes> {
        Buffer::Decoded(Bytes(value.to_vec()))
    }

    fn iovec(base: Buffer<Bytes>, len: u64) -> Iovec {
        Iovec { base, len }
    }

    #[test]
    fn display_entry() {
        assert_eq!(
            iovec(decoded(b"data"), 16).to_string(),
            "{iov_base=\"data\", iov_len=16}"
        );
        assert_eq!(
            iovec(Buffer::Address(0x7ffc1234), 8).to_string(),
            "{iov_base=0x7ffc1234, iov_len=8}"
        );
        assert_eq!(
            iovec(Buffer::Address(0), 0).to_string(),
            "{iov_base=NULL, iov_len=0}"
        );
    }

    #[test]
    fn display_list() {
        let list = IovecList(vec![iovec(decoded(b"abcd"), 4), iovec(decoded(b""), 8)]);
        assert_eq!(
            list.to_string(),
            "[{iov_base=\"abcd\", iov_len=4}, {iov_base=\"\", iov_len=8}]"
        );
        assert_eq!(IovecList(Vec::new()).to_string(), "[]");
        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
        assert_eq!(list.as_slice(), list.0.as_slice());
    }

    #[test]
    fn parse_entry() {
        assert_eq!(
            Iovec::from_str("{iov_base=\"data\", iov_len=16}").unwrap(),
            iovec(decoded(b"data"), 16)
        );
        assert_eq!(
            Iovec::from_str("{iov_base=0x7ffc1234, iov_len=8}").unwrap(),
            iovec(Buffer::Address(0x7ffc1234), 8)
        );
        assert_eq!(
            Iovec::from_str("{iov_base=NULL, iov_len=0}").unwrap(),
            iovec(Buffer::Address(0), 0)
        );
    }

    #[test]
    fn parse_list() {
        assert_eq!(
            IovecList::decode_buffer("[]").unwrap(),
            IovecList(Vec::new())
        );
        let list = IovecList::decode_buffer(
            "[{iov_base=\"abcd\", iov_len=4}, {iov_base=\"\", iov_len=8}]",
        )
        .unwrap();
        assert_eq!(
            list,
            IovecList(vec![iovec(decoded(b"abcd"), 4), iovec(decoded(b""), 8)])
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(Iovec::from_str("iov_base=0x1").is_err());
        assert!(Iovec::from_str("{iov_len=1}").is_err());
        assert!(Iovec::from_str("{iov_base=0x1}").is_err());
        assert!(Iovec::from_str("{iov_base=0x1, iov_len=x}").is_err());
        assert!(Iovec::from_str("{iov_base=0x1, iov_len=1, extra=2}").is_err());
        assert!(IovecList::decode_buffer("[{}").is_err());
        assert!(IovecList::decode_buffer("0x1").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "{iov_base=\"data\", iov_len=16}",
            "{iov_base=\"a, b\", iov_len=4}",
            "{iov_base=\"\", iov_len=8}",
            "{iov_base=NULL, iov_len=0}",
            "{iov_base=0x7ffc1234, iov_len=8}",
        ] {
            assert_eq!(Iovec::from_str(line).unwrap().to_string(), line);
        }
        let list = IovecList(vec![
            iovec(decoded(b"abcd"), 4),
            iovec(decoded(b""), 8),
            iovec(Buffer::Address(0x7ffc1234), 16),
        ]);
        let rendered = list.to_string();
        assert_eq!(IovecList::decode_buffer(&rendered).unwrap(), list);
    }
}
