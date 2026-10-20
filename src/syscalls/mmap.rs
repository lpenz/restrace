// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`mmap(2)`] system call.
//!
//! [`mmap(2)`]: https://man7.org/linux/man-pages/man2/mmap.2.html

use super::errno::Errno;
use super::fd::Fd;
use super::flags;
use super::parse::{self, ParseError};
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::str::FromStr;

/// The low bits of the `flags` argument that select the mapping type.
const MAP_TYPE: u32 = 0x0f;

/// The shift and mask of the huge-page size encoded in the `flags` argument.
const MAP_HUGE_SHIFT: u32 = 26;
const MAP_HUGE_MASK: u32 = 0x3f;

/// The huge-page size bits of the `flags` argument.
const MAP_HUGE_BITS: u32 = MAP_HUGE_MASK << MAP_HUGE_SHIFT;

/// The mapping-type names `strace` prints for the [`MAP_TYPE`] bits.
const TYPE_NAMES: &[(u32, &str)] = &[
    (0, "MAP_FILE"),
    (1, "MAP_SHARED"),
    (2, "MAP_PRIVATE"),
    (3, "MAP_SHARED_VALIDATE"),
    (8, "MAP_DROPPABLE"),
];

/// The remaining named bits of the `flags` argument, in the order `strace`
/// prints them.
const REST_NAMES: &[(u32, &str)] = &[
    (0x10, "MAP_FIXED"),
    (0x20, "MAP_ANONYMOUS"),
    (0x40, "MAP_32BIT"),
    (0x4000, "MAP_NORESERVE"),
    (0x8000, "MAP_POPULATE"),
    (0x10000, "MAP_NONBLOCK"),
    (0x100, "MAP_GROWSDOWN"),
    (0x800, "MAP_DENYWRITE"),
    (0x1000, "MAP_EXECUTABLE"),
    (0x2000, "MAP_LOCKED"),
    (0x20000, "MAP_STACK"),
    (0x40000, "MAP_HUGETLB"),
    (0x80000, "MAP_SYNC"),
    (0x100000, "MAP_FIXED_NOREPLACE"),
];

/// Named bits of [`ProtFlags`], in [`Display`](fmt::Display) order.
const PROT_NAMES: &[(u32, &str)] = &[
    (ProtFlags::PROT_NONE.0, "PROT_NONE"),
    (ProtFlags::PROT_READ.0, "PROT_READ"),
    (ProtFlags::PROT_WRITE.0, "PROT_WRITE"),
    (ProtFlags::PROT_EXEC.0, "PROT_EXEC"),
    (ProtFlags::PROT_SEM.0, "PROT_SEM"),
    (ProtFlags::PROT_GROWSDOWN.0, "PROT_GROWSDOWN"),
    (ProtFlags::PROT_GROWSUP.0, "PROT_GROWSUP"),
];

/// Parses a value `strace` prints with `%#llx`: `0x`-prefixed hexadecimal,
/// or a bare decimal number such as the zero it prints for a zero value.
fn parse_hex_arg(s: &str, part: &'static str) -> Result<u64, ParseError> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|_| ParseError::new(part, s))
    } else {
        s.parse::<u64>().map_err(|_| ParseError::new(part, s))
    }
}

/// Arguments and return value of the `mmap(2)` system call.
///
/// ```text
/// void *mmap(void *addr, size_t length, int prot, int flags,
///            int fd, off_t offset);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mmap {
    /// Address hint; zero, which `strace` prints as `NULL`, lets the kernel
    /// choose the address.
    pub addr: u64,
    /// Length of the mapping in bytes.
    pub length: u64,
    /// Protection of the mapping; see [`ProtFlags`].
    pub prot: ProtFlags,
    /// Type and options of the mapping; see [`MmapFlags`].
    pub flags: MmapFlags,
    /// File descriptor the mapping maps from, or -1 for anonymous mappings,
    /// with the path `strace` resolved for it when run with `-y`.
    pub fd: Fd,
    /// Offset into the file, in bytes.
    pub offset: u64,
    /// Return value: the address of the mapped region, or the errno of the
    /// failure; see [`MapResult`].
    pub result: MapResult,
}

impl Mmap {
    /// Builds an `mmap(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the
    /// address of the mapped region on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        addr: u64,
        length: u64,
        prot: ProtFlags,
        flags: MmapFlags,
        fd: i32,
        offset: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            addr,
            length,
            prot,
            flags,
            fd: Fd::new(fd),
            offset,
            result: MapResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Mmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mmap(")?;
        if self.addr == 0 {
            f.write_str("NULL")?;
        } else {
            write!(f, "0x{:x}", self.addr)?;
        }
        write!(
            f,
            ", {}, {}, {}, {}, ",
            self.length, self.prot, self.flags, self.fd
        )?;
        if self.offset == 0 {
            f.write_str("0")?;
        } else {
            write!(f, "0x{:x}", self.offset)?;
        }
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, -1, 0)
/// = 0x7f...` or `... = -1 ENOMEM`.
impl FromStr for Mmap {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("mmap call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("mmap(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 6 {
            return Err(ParseError::new("mmap arguments", args));
        }
        let addr_part = parts[0].trim();
        let addr = if addr_part == "NULL" {
            0
        } else {
            parse_hex_arg(addr_part, "addr")?
        };
        let length = parts[1]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("length", parts[1]))?;
        let prot = ProtFlags::from_str(parts[2].trim())?;
        let flags = MmapFlags::from_str(parts[3].trim())?;
        let fd = parts[4]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[4]))?;
        let offset = parse_hex_arg(parts[5].trim(), "offset")?;
        Ok(Self {
            addr,
            length,
            prot,
            flags,
            fd,
            offset,
            result: MapResult::from_str(result)?,
        })
    }
}

/// The return value of `mmap(2)` and of the other calls that map memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapResult {
    /// The call succeeded; it returned this address.
    Address(u64),
    /// The call failed with this errno.
    Errno(Errno),
}

impl MapResult {
    /// Converts a raw return value into a [`MapResult`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// anything else is the address of the mapped region.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(Errno(-raw as i32))
        } else {
            Self::Address(raw.unsigned_abs())
        }
    }

    /// Returns the mapped address, if the call succeeded.
    #[must_use]
    pub const fn address(self) -> Option<u64> {
        match self {
            Self::Address(addr) => Some(addr),
            Self::Errno(_) => None,
        }
    }

    /// Returns true if the call succeeded.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Address(_))
    }
}

impl fmt::Display for MapResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(0) => f.write_str("0"),
            Self::Address(address) => write!(f, "0x{address:x}"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as [`Display`](fmt::Display) writes it: a mapped
/// address, or `-1` followed by the errno.
impl FromStr for MapResult {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Ok(Self::Errno(
                Errno::from_str(parse::strip_description(errno.trim())).map_err(|_| error())?,
            ));
        }
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            return u64::from_str_radix(hex, 16)
                .map(Self::Address)
                .map_err(|_| error());
        }
        s.parse::<u64>().map(Self::Address).map_err(|_| error())
    }
}

/// The `prot` argument of `mmap(2)`: the protection of the mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct ProtFlags(pub u32);

impl ProtFlags {
    /// No protection.
    pub const PROT_NONE: Self = Self(0);
    /// Pages can be read.
    pub const PROT_READ: Self = Self(0x1);
    /// Pages can be written.
    pub const PROT_WRITE: Self = Self(0x2);
    /// Pages can be executed.
    pub const PROT_EXEC: Self = Self(0x4);
    /// Read and write for semaphore-backed mappings.
    pub const PROT_SEM: Self = Self(0x8);
    /// Extends a stack mapping downward.
    pub const PROT_GROWSDOWN: Self = Self(0x01000000);
    /// Extends a stack mapping upward.
    pub const PROT_GROWSUP: Self = Self(0x02000000);
}

impl ProtFlags {
    /// Returns the bits of these flags that no known flag covers.
    #[must_use]
    pub const fn unknown_bits(self) -> u32 {
        self.0
            & !(Self::PROT_READ.0
                | Self::PROT_WRITE.0
                | Self::PROT_EXEC.0
                | Self::PROT_SEM.0
                | Self::PROT_GROWSDOWN.0
                | Self::PROT_GROWSUP.0)
    }
}

impl BitOr for ProtFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ProtFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ProtFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ProtFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for ProtFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl fmt::Display for ProtFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("PROT_NONE");
        }
        flags::display(f, self.0, PROT_NAMES, "PROT_???")
    }
}

/// Parses protection flags as [`Display`](fmt::Display) writes them.
impl FromStr for ProtFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, PROT_NAMES, "prot").map(Self)
    }
}

/// The `flags` argument of `mmap(2)`: the type and options of the mapping.
///
/// The low [`MAP_TYPE`] bits select the mapping type and are printed by name
/// first — `MAP_FILE` when they are zero — followed by the remaining named
/// bits joined by `|` and any huge-page size as `|N<<MAP_HUGE_SHIFT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct MmapFlags(pub u32);

impl MmapFlags {
    /// A file mapping, the default; equivalent to zero.
    pub const MAP_FILE: Self = Self(0);
    /// Share the mapping with all processes that map the same file.
    pub const MAP_SHARED: Self = Self(0x1);
    /// Create a private copy-on-write mapping.
    pub const MAP_PRIVATE: Self = Self(0x2);
    /// Like [`MAP_SHARED`](Self::MAP_SHARED), but validates the other flags.
    pub const MAP_SHARED_VALIDATE: Self = Self(0x3);
    /// Drop the pages when they leave memory (Linux 6.11 and later).
    pub const MAP_DROPPABLE: Self = Self(0x8);
    /// Use the exact address in the first argument, replacing any mapping
    /// already there.
    pub const MAP_FIXED: Self = Self(0x10);
    /// The mapping is not backed by a file.
    pub const MAP_ANONYMOUS: Self = Self(0x20);
    /// Allocate the mapping in the first 2 GiB of the address space
    /// (x86-specific).
    pub const MAP_32BIT: Self = Self(0x40);
    /// Do not reserve swap space for the mapping.
    pub const MAP_NORESERVE: Self = Self(0x4000);
    /// Populate page tables eagerly.
    pub const MAP_POPULATE: Self = Self(0x8000);
    /// Do not block on population.
    pub const MAP_NONBLOCK: Self = Self(0x10000);
    /// Grow a stack mapping down to this mapping's address.
    pub const MAP_GROWSDOWN: Self = Self(0x100);
    /// Deny write access to the underlying file (obsolete).
    pub const MAP_DENYWRITE: Self = Self(0x800);
    /// Require the file descriptor to be executable (obsolete).
    pub const MAP_EXECUTABLE: Self = Self(0x1000);
    /// Lock the pages of the mapping in memory.
    pub const MAP_LOCKED: Self = Self(0x2000);
    /// Act as a stack mapping.
    pub const MAP_STACK: Self = Self(0x20000);
    /// Use huge pages for the mapping.
    pub const MAP_HUGETLB: Self = Self(0x40000);
    /// Synchronize writes with the underlying file (DAX).
    pub const MAP_SYNC: Self = Self(0x80000);
    /// Like [`MAP_FIXED`](Self::MAP_FIXED), but fail if the range is already
    /// mapped instead of replacing it.
    pub const MAP_FIXED_NOREPLACE: Self = Self(0x100000);
}

impl MmapFlags {
    /// Returns the bits of these flags that no known flag covers.
    #[must_use]
    pub const fn unknown_bits(self) -> u32 {
        const KNOWN: u32 = MmapFlags::MAP_SHARED.0
            | MmapFlags::MAP_PRIVATE.0
            | MmapFlags::MAP_SHARED_VALIDATE.0
            | MmapFlags::MAP_DROPPABLE.0
            | MmapFlags::MAP_FIXED.0
            | MmapFlags::MAP_ANONYMOUS.0
            | MmapFlags::MAP_32BIT.0
            | MmapFlags::MAP_NORESERVE.0
            | MmapFlags::MAP_POPULATE.0
            | MmapFlags::MAP_NONBLOCK.0
            | MmapFlags::MAP_GROWSDOWN.0
            | MmapFlags::MAP_DENYWRITE.0
            | MmapFlags::MAP_EXECUTABLE.0
            | MmapFlags::MAP_LOCKED.0
            | MmapFlags::MAP_STACK.0
            | MmapFlags::MAP_HUGETLB.0
            | MmapFlags::MAP_SYNC.0
            | MmapFlags::MAP_FIXED_NOREPLACE.0
            | MAP_HUGE_BITS;
        self.0 & !KNOWN
    }
}

impl BitOr for MmapFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for MmapFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for MmapFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for MmapFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for MmapFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl fmt::Display for MmapFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0;
        // strace prints the mapping type from the low MAP_TYPE bits first,
        // by name or as MAP_FILE when the bits are zero.
        let type_bits = value & MAP_TYPE;
        match TYPE_NAMES.iter().find(|&&(v, _)| v == type_bits) {
            Some(&(_, name)) => f.write_str(name)?,
            None => write!(f, "0x{type_bits:x} /* MAP_??? */")?,
        }
        let rest = value & !MAP_TYPE & !MAP_HUGE_BITS;
        let huge = value & MAP_HUGE_BITS;
        if rest != 0 {
            let mut known = 0u32;
            for &(bit, name) in REST_NAMES {
                if rest & bit == bit {
                    write!(f, "|{name}")?;
                    known |= bit;
                }
            }
            let unknown = rest & !known;
            if unknown != 0 {
                write!(f, "|0x{unknown:x}")?;
            }
        }
        if huge != 0 {
            write!(f, "|{}<<MAP_HUGE_SHIFT", huge >> MAP_HUGE_SHIFT)?;
        }
        Ok(())
    }
}

/// Parses mapping flags as [`Display`](fmt::Display) writes them.
impl FromStr for MmapFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::new("flags", s));
        }
        let mut value = 0u32;
        for token in s.split('|') {
            let token = parse::strip_comment(token.trim());
            if token.is_empty() {
                return Err(ParseError::new("flags", s));
            }
            if let Some(&(bit, _)) = TYPE_NAMES
                .iter()
                .chain(REST_NAMES.iter())
                .find(|&&(_, name)| name == token)
            {
                value |= bit;
                continue;
            }
            if let Some((number, "MAP_HUGE_SHIFT")) = token.split_once("<<") {
                let size = number
                    .parse::<u32>()
                    .map_err(|_| ParseError::new("flags", token))?;
                if size > MAP_HUGE_MASK {
                    return Err(ParseError::new("flags", token));
                }
                value |= size << MAP_HUGE_SHIFT;
                continue;
            }
            let parsed = if let Some(hex) = token
                .strip_prefix("0x")
                .or_else(|| token.strip_prefix("0X"))
            {
                u32::from_str_radix(hex, 16)
            } else {
                token.parse::<u32>()
            };
            value |= parsed.map_err(|_| ParseError::new("flags", token))?;
        }
        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_success() {
        let mmap = Mmap::from_raw(
            0,
            8192,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MmapFlags::MAP_PRIVATE | MmapFlags::MAP_ANONYMOUS,
            -1,
            0,
            0x7f1234567000,
        );
        assert_eq!(
            mmap.to_string(),
            "mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, \
             -1, 0) = 0x7f1234567000"
        );
        assert!(mmap.result.is_ok());
        assert_eq!(mmap.result.address(), Some(0x7f1234567000));
    }

    #[test]
    fn display_failure() {
        let mmap = Mmap::from_raw(0, 0, ProtFlags::PROT_NONE, MmapFlags::MAP_FILE, 0, 0, -9);
        assert_eq!(
            mmap.to_string(),
            "mmap(NULL, 0, PROT_NONE, MAP_FILE, 0, 0) = -1 EBADF"
        );
        assert!(!mmap.result.is_ok());
        assert_eq!(mmap.result.address(), None);
    }

    #[test]
    fn parse_success() {
        let line = "mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, \
                    -1, 0) = 0x7f1234567000";
        let mmap = Mmap::from_str(line).unwrap();
        assert_eq!(mmap.addr, 0);
        assert_eq!(mmap.length, 8192);
        assert_eq!(mmap.prot, ProtFlags(0x3));
        assert_eq!(mmap.flags, MmapFlags(0x22));
        assert_eq!(mmap.fd, Fd::new(-1));
        assert_eq!(mmap.offset, 0);
        assert_eq!(mmap.result, MapResult::Address(0x7f1234567000));
        assert_eq!(
            mmap.to_string(),
            "mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, \
             -1, 0) = 0x7f1234567000"
        );
    }

    #[test]
    fn parse_failure() {
        let mmap = Mmap::from_str("mmap(NULL, 0, PROT_NONE, MAP_FILE, 0, 0) = -1 ENOMEM").unwrap();
        assert_eq!(mmap.addr, 0);
        assert_eq!(mmap.length, 0);
        assert_eq!(mmap.prot, ProtFlags::PROT_NONE);
        assert_eq!(mmap.flags, MmapFlags::MAP_FILE);
        assert_eq!(mmap.fd, Fd::new(0));
        assert_eq!(mmap.offset, 0);
        assert_eq!(mmap.result, MapResult::Errno(Errno::ENOMEM));
    }

    #[test]
    fn parse_described_failure() {
        let mmap = Mmap::from_str(
            "mmap(NULL, 0, PROT_NONE, MAP_FILE, 0, 0) = -1 EBADF (Bad file descriptor)",
        )
        .unwrap();
        assert_eq!(mmap.result, MapResult::Errno(Errno::EBADF));
        assert_eq!(
            mmap.to_string(),
            "mmap(NULL, 0, PROT_NONE, MAP_FILE, 0, 0) = -1 EBADF"
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(Mmap::from_str("mmap(NULL, 8192, PROT_READ, MAP_SHARED, -1, 0) 0").is_err());
        assert!(Mmap::from_str("mmap(NULL, 8192, PROT_READ, MAP_SHARED, -1) = 0").is_err());
        assert!(Mmap::from_str("mmap(x, 8192, PROT_READ, MAP_SHARED, -1, 0) = 0").is_err());
        assert!(Mmap::from_str("mmap(NULL, 8192, PROT_BOGUS, MAP_SHARED, -1, 0) = 0").is_err());
        assert!(Mmap::from_str("mmap(NULL, 8192, PROT_READ, MAP_BOGUS, -1, 0) = 0").is_err());
        assert!(Mmap::from_str("mprotect(NULL, 8192, PROT_READ) = 0").is_err());
        assert!(Mmap::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, \
             -1, 0) = 0x7f1234567000",
            "mmap(NULL, 0, PROT_NONE, MAP_FILE, 0, 0) = -1 EBADF",
            "mmap(0x7f0000000000, 4096, PROT_READ, MAP_SHARED, 3, 0x1000) = \
             0x7f0000001000",
            "mmap(NULL, 16384, PROT_NONE, MAP_FILE|MAP_FIXED, -2, 0xdecaffed) = \
             -1 ENOMEM",
            "mmap(NULL, 2097152, PROT_READ|PROT_EXEC, \
             MAP_PRIVATE|MAP_ANONYMOUS|MAP_STACK, -1, 0) = 0x7ffc00000000",
            "mmap(NULL, 4194304, PROT_READ, MAP_PRIVATE|MAP_ANONYMOUS|MAP_HUGETLB, \
             -1, 0) = 0x7f0000000000",
        ] {
            assert_eq!(Mmap::from_str(line).unwrap().to_string(), line);
        }
    }

    #[test]
    fn display_hugetlb_shift() {
        let flags = MmapFlags(MmapFlags::MAP_HUGETLB.0 | (21 << MAP_HUGE_SHIFT));
        assert_eq!(flags.to_string(), "MAP_FILE|MAP_HUGETLB|21<<MAP_HUGE_SHIFT");
        assert_eq!(
            MmapFlags::from_str("MAP_FILE|MAP_HUGETLB|21<<MAP_HUGE_SHIFT"),
            Ok(flags)
        );
    }

    #[test]
    fn display_unknown_type() {
        assert_eq!(MmapFlags(0x4).to_string(), "0x4 /* MAP_??? */");
        assert_eq!(MmapFlags::from_str("0x4 /* MAP_??? */"), Ok(MmapFlags(0x4)));
    }

    #[test]
    fn display_unknown_rest_bits() {
        assert_eq!(
            MmapFlags(MmapFlags::MAP_SHARED.0 | 0x400).to_string(),
            "MAP_SHARED|0x400"
        );
        assert_eq!(
            MmapFlags::from_str("MAP_SHARED|0x400"),
            Ok(MmapFlags(MmapFlags::MAP_SHARED.0 | 0x400))
        );
    }

    #[test]
    fn prot_display_and_parse() {
        assert_eq!(ProtFlags::PROT_NONE.to_string(), "PROT_NONE");
        assert_eq!(ProtFlags(0x3).to_string(), "PROT_READ|PROT_WRITE");
        assert_eq!(ProtFlags(0x80).to_string(), "0x80 /* PROT_??? */");
        assert_eq!(ProtFlags::from_str("PROT_NONE"), Ok(ProtFlags::PROT_NONE));
        assert_eq!(ProtFlags::from_str("0"), Ok(ProtFlags::PROT_NONE));
        assert_eq!(
            ProtFlags::from_str("PROT_READ|PROT_EXEC"),
            Ok(ProtFlags(0x5))
        );
        assert_eq!(
            ProtFlags::from_str("0x80 /* PROT_??? */"),
            Ok(ProtFlags(0x80))
        );
        assert!(ProtFlags::from_str("PROT_BOGUS").is_err());
    }

    #[test]
    fn unknown_bits() {
        assert_eq!(ProtFlags::PROT_READ.unknown_bits(), 0);
        assert_eq!(ProtFlags(0x80).unknown_bits(), 0x80);
        assert_eq!(MmapFlags::MAP_PRIVATE.unknown_bits(), 0);
        assert_eq!(MmapFlags(0x400).unknown_bits(), 0x400);
    }

    #[test]
    fn bitops() {
        let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
        assert_eq!(prot, ProtFlags(0x3));
        assert_eq!(prot & ProtFlags::PROT_READ, ProtFlags::PROT_READ);
        let flags = MmapFlags::MAP_PRIVATE | MmapFlags::MAP_ANONYMOUS;
        assert_eq!(flags, MmapFlags(0x22));
        assert_eq!(flags & MmapFlags::MAP_PRIVATE, MmapFlags::MAP_PRIVATE);
    }
}
