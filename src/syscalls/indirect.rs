// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Values that `strace` reads through a pointer and prints in square brackets.

use super::buffer::BufferValue;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// A scalar `strace` reads through a pointer and prints between square
/// brackets.
///
/// Some system calls take a pointer to an output scalar, such as the
/// `loff_t *` offsets of [`splice(2)`](super::splice::Splice). `strace`
/// reads the pointed-to value on entry and prints it as `[value]`. When the
/// call changes the value and `strace` reads the pointer again on exit, it
/// prints the new value too, as in `[0 => 10]`, which
/// [`Indirect::moved`] carries. Older `strace` releases (up to and including
/// 6.x) printed the two values in separate brackets, as `[0] => [10]`; the
/// parser accepts both, while `Display` always writes the newer form. Used
/// as the value type of a [`Buffer`](super::buffer::Buffer), the `NULL` and
/// raw address it prints when the pointer is null or cannot be read are
/// still handled by the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indirect<T> {
    /// The value `strace` read through the pointer on entry.
    pub value: T,
    /// The value the call left behind, when `strace` read the pointer again
    /// on exit and it differed from [`Indirect::value`].
    pub moved_to: Option<T>,
}

impl<T> Indirect<T> {
    /// Wraps a value the call did not change.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            value,
            moved_to: None,
        }
    }

    /// Wraps a value the call changed from `value` to `moved_to`.
    #[must_use]
    pub const fn moved(value: T, moved_to: T) -> Self {
        Self {
            value,
            moved_to: Some(moved_to),
        }
    }
}

impl<T: fmt::Display> fmt::Display for Indirect<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.moved_to {
            None => write!(f, "[{}]", self.value),
            Some(moved_to) => write!(f, "[{} => {}]", self.value, moved_to),
        }
    }
}

impl<T: FromStr> BufferValue for Indirect<T> {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        let error = || ParseError::new("indirect value", input);
        let trimmed = input.trim();
        // Old strace (≤ 6.x) printed each value in its own brackets, as
        // `[0] => [10]`; detect that before stripping the outer pair,
        // which would otherwise leave `0] => [10` behind.
        if let Some((left, right)) = trimmed.split_once("=>") {
            let left = left.trim();
            let right = right.trim();
            if let (Some(value), Some(moved_to)) = (
                left.strip_prefix('[').and_then(|s| s.strip_suffix(']')),
                right.strip_prefix('[').and_then(|s| s.strip_suffix(']')),
            ) {
                return Ok(Self::moved(
                    value.trim().parse::<T>().map_err(|_| error())?,
                    moved_to.trim().parse::<T>().map_err(|_| error())?,
                ));
            }
        }
        let inner = trimmed
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(error)?
            .trim();
        match inner.split_once("=>") {
            Some((value, moved_to)) => Ok(Self::moved(
                value.trim().parse::<T>().map_err(|_| error())?,
                moved_to.trim().parse::<T>().map_err(|_| error())?,
            )),
            None => Ok(Self::new(inner.parse::<T>().map_err(|_| error())?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::buffer::{self, Buffer};

    #[test]
    fn display() {
        assert_eq!(Indirect::new(5i64).to_string(), "[5]");
        assert_eq!(Indirect::new(-1i64).to_string(), "[-1]");
        assert_eq!(Indirect::moved(0u64, 10).to_string(), "[0 => 10]");
    }

    #[test]
    fn parse() {
        assert_eq!(Indirect::<i64>::decode_buffer("[5]"), Ok(Indirect::new(5)));
        assert_eq!(
            Indirect::<i64>::decode_buffer("[ 5 ]"),
            Ok(Indirect::new(5))
        );
        assert_eq!(
            Indirect::<u64>::decode_buffer("[0 => 10]"),
            Ok(Indirect::moved(0, 10))
        );
        assert_eq!(
            Indirect::<u64>::decode_buffer("[ 0 => 10 ]"),
            Ok(Indirect::moved(0, 10))
        );
        assert_eq!(
            Indirect::<u64>::decode_buffer("[0] => [10]"),
            Ok(Indirect::moved(0, 10))
        );
        assert_eq!(
            Indirect::<u64>::decode_buffer("[ 0 ] => [ 10 ]"),
            Ok(Indirect::moved(0, 10))
        );
        assert!(Indirect::<i64>::decode_buffer("5").is_err());
        assert!(Indirect::<i64>::decode_buffer("[x]").is_err());
        assert!(Indirect::<i64>::decode_buffer("[1 =>]").is_err());
        assert!(Indirect::<i64>::decode_buffer("[=> 1]").is_err());
        assert!(Indirect::<i64>::decode_buffer("[1] => []").is_err());
        assert!(Indirect::<i64>::decode_buffer("[] => [1]").is_err());
    }

    #[test]
    fn buffer_round_trip() {
        for line in ["[5]", "[-1]", "[0 => 10]", "NULL", "0x7ffc"] {
            let buffer: Buffer<Indirect<i64>> = buffer::parse(line).unwrap();
            assert_eq!(buffer.to_string(), line);
        }
    }
}
