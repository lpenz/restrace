// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `st_mode` symbolic file mode of a `stat(2)` result.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Mask of the file type bits.
const S_IFMT: u32 = 0o170000;
/// Set-user-ID on execution.
const S_ISUID: u32 = 0o4000;
/// Set-group-ID on execution.
const S_ISGID: u32 = 0o2000;
/// Sticky bit.
const S_ISVTX: u32 = 0o1000;

/// File type names of the `modetypes` table, indexed by the bits of
/// [`S_IFMT`].
const TYPES: &[(u32, &str)] = &[
    (0o140000, "S_IFSOCK"),
    (0o120000, "S_IFLNK"),
    (0o100000, "S_IFREG"),
    (0o060000, "S_IFBLK"),
    (0o040000, "S_IFDIR"),
    (0o020000, "S_IFCHR"),
    (0o010000, "S_IFIFO"),
];

/// The special permission bits, in the order `strace` prints them.
const SPECIAL: &[(u32, &str)] = &[
    (S_ISUID, "S_ISUID"),
    (S_ISGID, "S_ISGID"),
    (S_ISVTX, "S_ISVTX"),
];

/// Formats a value with the `%#03o` semantics of `strace`: octal with a
/// leading zero, zero-padded to at least three digits.
fn octal03(value: u32) -> String {
    let padded = format!("0{value:o}");
    if padded.len() >= 3 {
        padded
    } else {
        format!("{padded:0>3}")
    }
}

/// The `st_mode` field of a `struct stat`, printed symbolically.
///
/// Strace renders it as the file type (`S_IFREG`), the special permission
/// bits that are set (`S_ISUID`, `S_ISGID`, `S_ISVTX`) and the permission
/// bits in octal, joined by `|`, such as `S_IFREG|0644`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct FileMode(pub u32);

impl fmt::Display for FileMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ifmt = TYPES
            .iter()
            .find(|&&(bits, _)| bits == self.0 & S_IFMT)
            .map(|&(_, name)| name);
        let Some(ifmt) = ifmt else {
            return f.write_str(&octal03(self.0));
        };
        f.write_str(ifmt)?;
        f.write_str("|")?;
        for (bit, name) in SPECIAL {
            if self.0 & bit != 0 {
                f.write_str(name)?;
                f.write_str("|")?;
            }
        }
        f.write_str(&octal03(self.0 & !(S_IFMT | S_ISUID | S_ISGID | S_ISVTX)))
    }
}

/// Parses an octal mode with an optional `0o` prefix.
fn parse_octal(token: &str) -> Result<u32, ParseError> {
    let token = token.trim();
    let error = || ParseError::new("mode", token);
    let octal = token
        .strip_prefix("0o")
        .or_else(|| token.strip_prefix("0O"))
        .unwrap_or(token);
    u32::from_str_radix(octal, 8).map_err(|_| error())
}

/// Parses a symbolic mode as [`Display`](fmt::Display) writes it:
/// `S_IFREG|0644`, `S_IFCHR|S_ISUID|0666`, or just an octal mode when no
/// file type bit is set.
impl FromStr for FileMode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("mode", s);
        let Some((names, perms)) = s.rsplit_once('|') else {
            return parse_octal(s).map(Self);
        };
        let mut mode = parse_octal(perms)?;
        for token in names.split('|') {
            let token = token.trim();
            let bit = SPECIAL
                .iter()
                .find(|&&(_, name)| name == token)
                .map(|&(bit, _)| bit)
                .or_else(|| {
                    TYPES
                        .iter()
                        .find(|&&(_, name)| name == token)
                        .map(|&(bit, _)| bit)
                })
                .ok_or_else(error)?;
            mode |= bit;
        }
        Ok(Self(mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_type_and_perms() {
        assert_eq!(FileMode(0o100644).to_string(), "S_IFREG|0644");
        assert_eq!(FileMode(0o40755).to_string(), "S_IFDIR|0755");
        assert_eq!(FileMode(0o20666).to_string(), "S_IFCHR|0666");
    }

    #[test]
    fn display_special_bits() {
        assert_eq!(
            FileMode(0o107755).to_string(),
            "S_IFREG|S_ISUID|S_ISGID|S_ISVTX|0755"
        );
        assert_eq!(FileMode(0o140777).to_string(), "S_IFSOCK|0777");
    }

    #[test]
    fn display_no_type() {
        assert_eq!(FileMode(0o644).to_string(), "0644");
        assert_eq!(FileMode(0).to_string(), "000");
        assert_eq!(FileMode(0o4755).to_string(), "04755");
    }

    #[test]
    fn parse() {
        assert_eq!(FileMode::from_str("S_IFREG|0644"), Ok(FileMode(0o100644)));
        assert_eq!(FileMode::from_str("S_IFDIR|0755"), Ok(FileMode(0o40755)));
        assert_eq!(
            FileMode::from_str("S_IFREG|S_ISUID|S_ISGID|S_ISVTX|0755"),
            Ok(FileMode(0o107755))
        );
        assert_eq!(FileMode::from_str("0644"), Ok(FileMode(0o644)));
        assert_eq!(FileMode::from_str("0"), Ok(FileMode(0)));
        assert!(FileMode::from_str("S_IFREG").is_err());
        assert!(FileMode::from_str("S_IFREG|BOGUS").is_err());
        assert!(FileMode::from_str("S_BOGUS|0644").is_err());
        assert!(FileMode::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for mode in [
            FileMode(0),
            FileMode(0o100644),
            FileMode(0o40755),
            FileMode(0o140777),
            FileMode(0o107755),
            FileMode(0o644),
        ] {
            assert_eq!(FileMode::from_str(&mode.to_string()).unwrap(), mode);
        }
    }
}
