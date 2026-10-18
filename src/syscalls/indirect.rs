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
/// reads the pointed-to value on entry and prints it as `[value]`. Used as
/// the value type of a [`Buffer`](super::buffer::Buffer), the `NULL` and raw
/// address it prints when the pointer is null or cannot be read are still
/// handled by the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indirect<T>(pub T);

impl<T: fmt::Display> fmt::Display for Indirect<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.0)
    }
}

impl<T: FromStr> BufferValue for Indirect<T> {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        let error = || ParseError::new("indirect value", input);
        let inner = input
            .trim()
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(error)?;
        inner.trim().parse::<T>().map(Self).map_err(|_| error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::buffer::{self, Buffer};

    #[test]
    fn display() {
        assert_eq!(Indirect(5i64).to_string(), "[5]");
        assert_eq!(Indirect(-1i64).to_string(), "[-1]");
    }

    #[test]
    fn parse() {
        assert_eq!(Indirect::<i64>::decode_buffer("[5]"), Ok(Indirect(5)));
        assert_eq!(Indirect::<i64>::decode_buffer("[ 5 ]"), Ok(Indirect(5)));
        assert!(Indirect::<i64>::decode_buffer("5").is_err());
        assert!(Indirect::<i64>::decode_buffer("[x]").is_err());
    }

    #[test]
    fn buffer_round_trip() {
        for line in ["[5]", "[-1]", "NULL", "0x7ffc"] {
            let buffer: Buffer<Indirect<i64>> = buffer::parse(line).unwrap();
            assert_eq!(buffer.to_string(), line);
        }
    }
}
