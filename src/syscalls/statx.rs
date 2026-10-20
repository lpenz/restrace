// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`statx(2)`] system call.
//!
//! [`statx(2)`]: https://man7.org/linux/man-pages/man2/statx.2.html

use super::buffer::Buffer;
use super::dirfd::DirFd;
use super::filemode::FileMode;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `statx(2)` system call.
///
/// ```text
/// int statx(int dirfd, const char *restrict pathname, int flags,
///           unsigned int mask, struct statx *restrict statxbuf);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statx {
    /// Directory file descriptor.
    pub dirfd: DirFd,
    /// Pathname of the file to stat, without the terminating NUL byte.
    pub pathname: CString,
    /// The `flags` argument; see [`StatxFlags`].
    pub flags: StatxFlags,
    /// The `mask` argument: which fields the caller is interested in; see
    /// [`StatxMask`].
    pub mask: StatxMask,
    /// The `struct statx` the call filled in, or the raw address of the
    /// buffer when `strace` could not decode it, such as after a failure.
    pub buf: Buffer<StatxBuf>,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Statx {
    /// Builds a `statx(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw(
        dirfd: DirFd,
        pathname: impl AsRef<[u8]>,
        flags: StatxFlags,
        mask: StatxMask,
        buf: Buffer<StatxBuf>,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            flags,
            mask,
            buf,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Statx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "statx({}, {:?}, {}, {}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.flags,
            self.mask,
            self.buf,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `statx(AT_FDCWD, "path", AT_STATX_SYNC_AS_STAT, STATX_ALL, {...}) = 0`.
impl FromStr for Statx {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("statx call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("statx(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 5 {
            return Err(ParseError::new("statx arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let flags = StatxFlags::from_str(parts[2].trim())?;
        let mask = StatxMask::from_str(parts[3].trim())?;
        let buf = super::buffer::parse(parts[4].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            flags,
            mask,
            buf,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `AT_STATX_SYNC_TYPE` bits of the `statx(2)` flags argument.
const AT_STATX_SYNC_TYPE: u32 = 0x6000;

/// The `AT_STATX_SYNC_TYPE` names of [`StatxFlags`], in
/// [`Display`](fmt::Display) order.
///
/// The table has no entry for `AT_STATX_SYNC_AS_STAT` (zero): `strace` prints
/// that name when the sync-type bits are zero, which [`Display`](fmt::Display)
/// reproduces explicitly.
const SYNC_NAMES: &[(u32, &str)] = &[
    (0x2000, "AT_STATX_FORCE_SYNC"),
    (0x4000, "AT_STATX_DONT_SYNC"),
];

/// The remaining `at_flags` names of [`StatxFlags`], the table `strace` uses
/// for the bits outside [`AT_STATX_SYNC_TYPE`].
const AT_FLAG_NAMES: &[(u32, &str)] = &[
    (0x100, "AT_SYMLINK_NOFOLLOW"),
    (0x200, "AT_REMOVEDIR"),
    (0x400, "AT_SYMLINK_FOLLOW"),
    (0x800, "AT_NO_AUTOMOUNT"),
    (0x1000, "AT_EMPTY_PATH"),
    (0x8000, "AT_RECURSIVE"),
];

/// All the flag names of [`StatxFlags`], for parsing: the sync-type names and
/// the `at_flags` names, plus the zero-valued `AT_STATX_SYNC_AS_STAT`.
const ALL_FLAG_NAMES: &[(u32, &str)] = &[
    (0, "AT_STATX_SYNC_AS_STAT"),
    (0x2000, "AT_STATX_FORCE_SYNC"),
    (0x4000, "AT_STATX_DONT_SYNC"),
    (0x100, "AT_SYMLINK_NOFOLLOW"),
    (0x200, "AT_REMOVEDIR"),
    (0x400, "AT_SYMLINK_FOLLOW"),
    (0x800, "AT_NO_AUTOMOUNT"),
    (0x1000, "AT_EMPTY_PATH"),
    (0x8000, "AT_RECURSIVE"),
];

/// The `flags` argument of the `statx(2)` call.
///
/// `strace` prints the `AT_STATX_SYNC_TYPE` bits first, from its sync-type
/// table, and the remaining `at_flags` bits after a `|`; when the sync-type
/// bits are zero it prints `AT_STATX_SYNC_AS_STAT`, the zero entry of the
/// sync-type table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct StatxFlags(pub u32);

impl StatxFlags {
    /// Let the filesystem serve the request without forcing or delaying a
    /// flush to disk; the zero sync type.
    pub const AT_STATX_SYNC_AS_STAT: Self = Self(0);
    /// Force the filesystem to complete the request before returning.
    pub const AT_STATX_FORCE_SYNC: Self = Self(0x2000);
    /// Serve the request from cached data without forcing a flush.
    pub const AT_STATX_DONT_SYNC: Self = Self(0x4000);
}

impl fmt::Display for StatxFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sync = self.0 & AT_STATX_SYNC_TYPE;
        let rest = self.0 & !AT_STATX_SYNC_TYPE;
        if sync == 0 {
            f.write_str("AT_STATX_SYNC_AS_STAT")?;
        } else {
            let mut separator = "";
            for &(bit, name) in SYNC_NAMES {
                if sync & bit == bit {
                    write!(f, "{separator}{name}")?;
                    separator = "|";
                }
            }
        }
        if rest != 0 {
            f.write_str("|")?;
            flags::display(f, rest, AT_FLAG_NAMES, "")?;
        }
        Ok(())
    }
}

/// Parses a flags argument as [`Display`](fmt::Display) writes it: the
/// sync-type name, optionally followed by `|` and the `at_flags` names.
impl FromStr for StatxFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        Ok(Self(flags::parse(s, ALL_FLAG_NAMES, "statx flags")?))
    }
}

/// The `mask` argument of the `statx(2)` call, and the `stx_mask` field of
/// the buffer it fills in: which information the caller is interested in, or
/// which information is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct StatxMask(pub u32);

impl StatxMask {
    /// Ask for every field the kernel knows about.
    pub const STATX_ALL: Self = Self(0x00000fff);
    /// Ask for the basic file metadata (type, mode, nlink, ids, size, blocks
    /// and the three timestamps).
    pub const STATX_BASIC_STATS: Self = Self(0x000007ff);
}

impl fmt::Display for StatxMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, MASK_NAMES, "STATX_???")
    }
}

impl FromStr for StatxMask {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        Ok(Self(flags::parse(s, MASK_NAMES, "statx mask")?))
    }
}

/// The names of the [`StatxMask`] bits, in the order `strace` prints them.
///
/// These are the names of strace's `statx_masks` table; the composite entries
/// `STATX_ALL` and `STATX_BASIC_STATS` come first so that
/// [`flags::display`] prints them instead of the individual bits they cover.
const MASK_NAMES: &[(u32, &str)] = &[
    (0x00000fff, "STATX_ALL"),
    (0x000007ff, "STATX_BASIC_STATS"),
    (0x00000001, "STATX_TYPE"),
    (0x00000002, "STATX_MODE"),
    (0x00000004, "STATX_NLINK"),
    (0x00000008, "STATX_UID"),
    (0x00000010, "STATX_GID"),
    (0x00000020, "STATX_ATIME"),
    (0x00000040, "STATX_MTIME"),
    (0x00000080, "STATX_CTIME"),
    (0x00000100, "STATX_INO"),
    (0x00000200, "STATX_SIZE"),
    (0x00000400, "STATX_BLOCKS"),
    (0x00000800, "STATX_BTIME"),
    (0x00001000, "STATX_MNT_ID"),
    (0x00002000, "STATX_DIOALIGN"),
    (0x00004000, "STATX_MNT_ID_UNIQUE"),
    (0x00008000, "STATX_SUBVOL"),
    (0x00010000, "STATX_WRITE_ATOMIC"),
    (0x00020000, "STATX_DIO_READ_ALIGN"),
];

/// The `stx_attributes` and `stx_attributes_mask` fields of
/// [`StatxBuf`](super::statx::StatxBuf): filesystem-specific inode flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct StatxAttrFlags(pub u32);

impl fmt::Display for StatxAttrFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, ATTR_NAMES, "STATX_ATTR_???")
    }
}

impl FromStr for StatxAttrFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        Ok(Self(flags::parse(s, ATTR_NAMES, "statx attributes")?))
    }
}

/// The names of the [`StatxAttrFlags`] bits, in the order `strace` prints
/// them.
const ATTR_NAMES: &[(u32, &str)] = &[
    (0x00000004, "STATX_ATTR_COMPRESSED"),
    (0x00000010, "STATX_ATTR_IMMUTABLE"),
    (0x00000020, "STATX_ATTR_APPEND"),
    (0x00000040, "STATX_ATTR_NODUMP"),
    (0x00000800, "STATX_ATTR_ENCRYPTED"),
    (0x00001000, "STATX_ATTR_AUTOMOUNT"),
    (0x00002000, "STATX_ATTR_MOUNT_ROOT"),
    (0x00100000, "STATX_ATTR_VERITY"),
    (0x00200000, "STATX_ATTR_DAX"),
    (0x00400000, "STATX_ATTR_WRITE_ATOMIC"),
];

/// A timestamp field of [`StatxBuf`], the `struct statx_timestamp` the kernel
/// returns.
///
/// The human-readable date in the ` /* ... */` comment `strace` appends is
/// not stored: like the errno description, it is derived from the value and
/// dropped when parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatxTimestamp {
    /// Seconds since the Unix epoch, the `tv_sec` field.
    pub tv_sec: i64,
    /// Nanoseconds of the timestamp, the `tv_nsec` field.
    pub tv_nsec: u64,
}

impl StatxTimestamp {
    /// Builds a timestamp from its seconds and nanoseconds fields.
    #[must_use]
    pub const fn from_raw(tv_sec: i64, tv_nsec: u64) -> Self {
        Self { tv_sec, tv_nsec }
    }
}

impl fmt::Display for StatxTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{tv_sec={}, tv_nsec={}}}", self.tv_sec, self.tv_nsec)
    }
}

/// Parses a timestamp as [`Display`](fmt::Display) writes it, dropping the
/// ` /* ... */` date comment `strace` appends.
impl FromStr for StatxTimestamp {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = parse::strip_comment(s.trim());
        let error = || ParseError::new("statx timestamp", s);
        let body = s
            .strip_prefix("{")
            .and_then(|s| s.strip_suffix("}"))
            .ok_or_else(error)?;
        let mut tv_sec = None;
        let mut tv_nsec = None;
        for field in parse::split_args_nested(body) {
            let (key, value) = field.split_once('=').ok_or_else(error)?;
            match key.trim() {
                "tv_sec" => {
                    tv_sec = Some(
                        value
                            .trim()
                            .parse::<i64>()
                            .map_err(|_| ParseError::new("tv_sec", value))?,
                    );
                }
                "tv_nsec" => {
                    tv_nsec = Some(
                        value
                            .trim()
                            .parse::<u64>()
                            .map_err(|_| ParseError::new("tv_nsec", value))?,
                    );
                }
                _ => return Err(ParseError::new("statx timestamp field", field)),
            }
        }
        Ok(Self {
            tv_sec: tv_sec.ok_or_else(error)?,
            tv_nsec: tv_nsec.ok_or_else(error)?,
        })
    }
}

/// Parses an unsigned decimal field.
fn parse_u64(input: &str, part: &'static str) -> Result<u64, ParseError> {
    input
        .trim()
        .parse::<u64>()
        .map_err(|_| ParseError::new(part, input))
}

/// Parses an unsigned 32-bit decimal field.
fn parse_u32(input: &str, part: &'static str) -> Result<u32, ParseError> {
    input
        .trim()
        .parse::<u32>()
        .map_err(|_| ParseError::new(part, input))
}

/// Parses a field `strace` prints in hexadecimal.
fn parse_hex(input: &str, part: &'static str) -> Result<u64, ParseError> {
    let input = input.trim();
    let hex = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .ok_or_else(|| ParseError::new(part, input))?;
    u64::from_str_radix(hex, 16).map_err(|_| ParseError::new(part, input))
}

/// The `struct statx` a `statx(2)` call filled in.
///
/// Strace prints it with `-v`, which the crate always passes, so all the
/// fields are shown. Which fields are present depends on the bits of
/// [`mask`](Self::mask) and [`attributes`](Self::attributes): the fields the
/// kernel left out are `None`, as are the fields a kernel too old to know
/// them would leave out. The `/* date */` comments that carry a
/// human-readable rendering of the timestamps are not stored, like the errno
/// descriptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatxBuf {
    /// Which fields are valid, the `stx_mask` field.
    pub mask: StatxMask,
    /// Preferred block size for filesystem I/O, the `stx_blksize` field.
    pub blksize: u64,
    /// Inode flags, the `stx_attributes` field.
    pub attributes: StatxAttrFlags,
    /// Number of hard links, the `stx_nlink` field; present with
    /// `STATX_NLINK`.
    pub nlink: Option<u64>,
    /// Owner user ID, the `stx_uid` field; present with `STATX_UID`.
    pub uid: Option<u32>,
    /// Owner group ID, the `stx_gid` field; present with `STATX_GID`.
    pub gid: Option<u32>,
    /// Symbolic file mode, the `stx_mode` field; present with `STATX_TYPE` or
    /// `STATX_MODE`.
    pub mode: Option<FileMode>,
    /// Inode number, the `stx_ino` field; present with `STATX_INO`.
    pub ino: Option<u64>,
    /// Total size in bytes, the `stx_size` field; present with `STATX_SIZE`.
    pub size: Option<u64>,
    /// Number of 512-byte blocks allocated, the `stx_blocks` field; present
    /// with `STATX_BLOCKS`.
    pub blocks: Option<u64>,
    /// Which inode flags the filesystem supports and tracks, the
    /// `stx_attributes_mask` field.
    pub attributes_mask: StatxAttrFlags,
    /// Last access time, the `stx_atime` field; present with `STATX_ATIME`.
    pub atime: Option<StatxTimestamp>,
    /// Last birth (creation) time, the `stx_btime` field; present with
    /// `STATX_BTIME`.
    pub btime: Option<StatxTimestamp>,
    /// Last status change time, the `stx_ctime` field; present with
    /// `STATX_CTIME`.
    pub ctime: Option<StatxTimestamp>,
    /// Last modification time, the `stx_mtime` field; present with
    /// `STATX_MTIME`.
    pub mtime: Option<StatxTimestamp>,
    /// Major device number of the device the file resides on, the
    /// `stx_rdev_major` field.
    pub rdev_major: u64,
    /// Minor device number of the device the file resides on, the
    /// `stx_rdev_minor` field.
    pub rdev_minor: u64,
    /// Major device number of the filesystem, the `stx_dev_major` field.
    pub dev_major: u64,
    /// Minor device number of the filesystem, the `stx_dev_minor` field.
    pub dev_minor: u64,
    /// Mount ID of the filesystem the file resides on, the `stx_mnt_id`
    /// field; present with `STATX_MNT_ID`.
    pub mnt_id: Option<u64>,
    /// In-memory I/O alignment, the `stx_dio_mem_align` field; present with
    /// `STATX_DIOALIGN`.
    pub dio_mem_align: Option<u64>,
    /// Offset alignment for direct I/O, the `stx_dio_offset_align` field;
    /// present with `STATX_DIOALIGN`.
    pub dio_offset_align: Option<u64>,
    /// Btrfs subvolume the file belongs to, the `stx_subvol` field; present
    /// with `STATX_SUBVOL`.
    pub subvol: Option<u64>,
    /// Smallest possible atomic write unit, the `stx_atomic_write_unit_min`
    /// field; present with `STATX_ATTR_WRITE_ATOMIC`.
    pub atomic_write_unit_min: Option<u64>,
    /// Largest possible atomic write unit, the `stx_atomic_write_unit_max`
    /// field; present with `STATX_ATTR_WRITE_ATOMIC`.
    pub atomic_write_unit_max: Option<u64>,
    /// Largest possible atomic write in segments, the
    /// `stx_atomic_write_segments_max` field; present with
    /// `STATX_ATTR_WRITE_ATOMIC`.
    pub atomic_write_segments_max: Option<u64>,
    /// Offset alignment for read I/O, the `stx_dio_read_offset_align` field;
    /// present with `STATX_DIO_READ_ALIGN`.
    pub dio_read_offset_align: Option<u64>,
    /// Largest optimal atomic write unit, the `stx_atomic_write_unit_max_opt`
    /// field; present with `STATX_ATTR_WRITE_ATOMIC` on newer kernels.
    pub atomic_write_unit_max_opt: Option<u64>,
}

impl fmt::Display for StatxBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{stx_mask={}, stx_blksize={}, stx_attributes={}",
            self.mask, self.blksize, self.attributes
        )?;
        if let Some(nlink) = self.nlink {
            write!(f, ", stx_nlink={nlink}")?;
        }
        if let Some(uid) = self.uid {
            write!(f, ", stx_uid={uid}")?;
        }
        if let Some(gid) = self.gid {
            write!(f, ", stx_gid={gid}")?;
        }
        if let Some(mode) = self.mode {
            write!(f, ", stx_mode={mode}")?;
        }
        if let Some(ino) = self.ino {
            write!(f, ", stx_ino={ino}")?;
        }
        if let Some(size) = self.size {
            write!(f, ", stx_size={size}")?;
        }
        if let Some(blocks) = self.blocks {
            write!(f, ", stx_blocks={blocks}")?;
        }
        write!(f, ", stx_attributes_mask={}", self.attributes_mask)?;
        if let Some(atime) = self.atime {
            write!(f, ", stx_atime={atime}")?;
        }
        if let Some(btime) = self.btime {
            write!(f, ", stx_btime={btime}")?;
        }
        if let Some(ctime) = self.ctime {
            write!(f, ", stx_ctime={ctime}")?;
        }
        if let Some(mtime) = self.mtime {
            write!(f, ", stx_mtime={mtime}")?;
        }
        write!(
            f,
            ", stx_rdev_major={}, stx_rdev_minor={}, stx_dev_major={}, stx_dev_minor={}",
            self.rdev_major, self.rdev_minor, self.dev_major, self.dev_minor
        )?;
        if let Some(mnt_id) = self.mnt_id {
            write!(f, ", stx_mnt_id=0x{mnt_id:x}")?;
        }
        if let (Some(dio_mem_align), Some(dio_offset_align)) =
            (self.dio_mem_align, self.dio_offset_align)
        {
            write!(
                f,
                ", stx_dio_mem_align={dio_mem_align}, stx_dio_offset_align={dio_offset_align}"
            )?;
        }
        if let Some(subvol) = self.subvol {
            write!(f, ", stx_subvol=0x{subvol:x}")?;
        }
        if let (Some(unit_min), Some(unit_max), Some(segments_max)) = (
            self.atomic_write_unit_min,
            self.atomic_write_unit_max,
            self.atomic_write_segments_max,
        ) {
            write!(
                f,
                ", stx_atomic_write_unit_min={unit_min}, stx_atomic_write_unit_max={unit_max}, \
                 stx_atomic_write_segments_max={segments_max}"
            )?;
        }
        if let Some(dio_read_offset_align) = self.dio_read_offset_align {
            write!(f, ", stx_dio_read_offset_align={dio_read_offset_align}")?;
        }
        if let Some(unit_max_opt) = self.atomic_write_unit_max_opt {
            write!(f, ", stx_atomic_write_unit_max_opt={unit_max_opt}")?;
        }
        f.write_str("}")
    }
}

/// Parses a `struct statx` as [`Display`](fmt::Display) writes it:
/// `{stx_mask=STATX_ALL, stx_blksize=4096, ...}`.
impl FromStr for StatxBuf {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("statx buffer", s);
        let body = s
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .ok_or_else(error)?;
        let mut mask = None;
        let mut blksize = None;
        let mut attributes = None;
        let mut nlink = None;
        let mut uid = None;
        let mut gid = None;
        let mut mode = None;
        let mut ino = None;
        let mut size = None;
        let mut blocks = None;
        let mut attributes_mask = None;
        let mut atime = None;
        let mut btime = None;
        let mut ctime = None;
        let mut mtime = None;
        let mut rdev_major = None;
        let mut rdev_minor = None;
        let mut dev_major = None;
        let mut dev_minor = None;
        let mut mnt_id = None;
        let mut dio_mem_align = None;
        let mut dio_offset_align = None;
        let mut subvol = None;
        let mut atomic_write_unit_min = None;
        let mut atomic_write_unit_max = None;
        let mut atomic_write_segments_max = None;
        let mut dio_read_offset_align = None;
        let mut atomic_write_unit_max_opt = None;
        for field in parse::split_args_nested(body) {
            let (key, value) = field.split_once('=').ok_or_else(error)?;
            match key.trim() {
                "stx_mask" => mask = Some(StatxMask::from_str(value)?),
                "stx_blksize" => blksize = Some(parse_u64(value, "stx_blksize")?),
                "stx_attributes" => attributes = Some(StatxAttrFlags::from_str(value)?),
                "stx_nlink" => nlink = Some(parse_u64(value, "stx_nlink")?),
                "stx_uid" => uid = Some(parse_u32(value, "stx_uid")?),
                "stx_gid" => gid = Some(parse_u32(value, "stx_gid")?),
                "stx_mode" => mode = Some(FileMode::from_str(value.trim())?),
                "stx_ino" => ino = Some(parse_u64(value, "stx_ino")?),
                "stx_size" => size = Some(parse_u64(value, "stx_size")?),
                "stx_blocks" => blocks = Some(parse_u64(value, "stx_blocks")?),
                "stx_attributes_mask" => {
                    attributes_mask = Some(StatxAttrFlags::from_str(value)?);
                }
                "stx_atime" => atime = Some(StatxTimestamp::from_str(value)?),
                "stx_btime" => btime = Some(StatxTimestamp::from_str(value)?),
                "stx_ctime" => ctime = Some(StatxTimestamp::from_str(value)?),
                "stx_mtime" => mtime = Some(StatxTimestamp::from_str(value)?),
                "stx_rdev_major" => rdev_major = Some(parse_u64(value, "stx_rdev_major")?),
                "stx_rdev_minor" => rdev_minor = Some(parse_u64(value, "stx_rdev_minor")?),
                "stx_dev_major" => dev_major = Some(parse_u64(value, "stx_dev_major")?),
                "stx_dev_minor" => dev_minor = Some(parse_u64(value, "stx_dev_minor")?),
                "stx_mnt_id" => mnt_id = Some(parse_hex(value, "stx_mnt_id")?),
                "stx_dio_mem_align" => dio_mem_align = Some(parse_u64(value, "stx_dio_mem_align")?),
                "stx_dio_offset_align" => {
                    dio_offset_align = Some(parse_u64(value, "stx_dio_offset_align")?);
                }
                "stx_subvol" => subvol = Some(parse_hex(value, "stx_subvol")?),
                "stx_atomic_write_unit_min" => {
                    atomic_write_unit_min = Some(parse_u64(value, "stx_atomic_write_unit_min")?);
                }
                "stx_atomic_write_unit_max" => {
                    atomic_write_unit_max = Some(parse_u64(value, "stx_atomic_write_unit_max")?);
                }
                "stx_atomic_write_segments_max" => {
                    atomic_write_segments_max =
                        Some(parse_u64(value, "stx_atomic_write_segments_max")?);
                }
                "stx_dio_read_offset_align" => {
                    dio_read_offset_align = Some(parse_u64(value, "stx_dio_read_offset_align")?);
                }
                "stx_atomic_write_unit_max_opt" => {
                    atomic_write_unit_max_opt =
                        Some(parse_u64(value, "stx_atomic_write_unit_max_opt")?);
                }
                _ => return Err(ParseError::new("statx field", field)),
            }
        }
        Ok(Self {
            mask: mask.ok_or_else(error)?,
            blksize: blksize.ok_or_else(error)?,
            attributes: attributes.ok_or_else(error)?,
            nlink,
            uid,
            gid,
            mode,
            ino,
            size,
            blocks,
            attributes_mask: attributes_mask.ok_or_else(error)?,
            atime,
            btime,
            ctime,
            mtime,
            rdev_major: rdev_major.ok_or_else(error)?,
            rdev_minor: rdev_minor.ok_or_else(error)?,
            dev_major: dev_major.ok_or_else(error)?,
            dev_minor: dev_minor.ok_or_else(error)?,
            mnt_id,
            dio_mem_align,
            dio_offset_align,
            subvol,
            atomic_write_unit_min,
            atomic_write_unit_max,
            atomic_write_segments_max,
            dio_read_offset_align,
            atomic_write_unit_max_opt,
        })
    }
}

impl super::buffer::BufferValue for StatxBuf {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        Self::from_str(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    const REGULAR: &str = "{stx_mask=STATX_ALL|STATX_MNT_ID|STATX_SUBVOL, stx_blksize=4096, \
        stx_attributes=0, stx_nlink=1, stx_uid=0, stx_gid=0, stx_mode=S_IFREG|0644, \
        stx_ino=6997, stx_size=8, stx_blocks=8, \
        stx_attributes_mask=STATX_ATTR_COMPRESSED|STATX_ATTR_IMMUTABLE|STATX_ATTR_APPEND|\
        STATX_ATTR_NODUMP|STATX_ATTR_AUTOMOUNT|STATX_ATTR_MOUNT_ROOT|STATX_ATTR_DAX, \
        stx_atime={tv_sec=1754766014, tv_nsec=986788270}, \
        stx_btime={tv_sec=1754766014, tv_nsec=986788270}, \
        stx_ctime={tv_sec=1754766014, tv_nsec=986788270}, \
        stx_mtime={tv_sec=1545480610, tv_nsec=541614343}, \
        stx_rdev_major=0, stx_rdev_minor=0, stx_dev_major=0, stx_dev_minor=30, \
        stx_mnt_id=0x20, stx_subvol=0x100}";

    fn regular() -> StatxBuf {
        StatxBuf {
            mask: StatxMask(0x00009fff),
            blksize: 4096,
            attributes: StatxAttrFlags(0),
            nlink: Some(1),
            uid: Some(0),
            gid: Some(0),
            mode: Some(FileMode(0o100644)),
            ino: Some(6997),
            size: Some(8),
            blocks: Some(8),
            attributes_mask: StatxAttrFlags(0x00203074),
            atime: Some(StatxTimestamp::from_raw(1754766014, 986788270)),
            btime: Some(StatxTimestamp::from_raw(1754766014, 986788270)),
            ctime: Some(StatxTimestamp::from_raw(1754766014, 986788270)),
            mtime: Some(StatxTimestamp::from_raw(1545480610, 541614343)),
            rdev_major: 0,
            rdev_minor: 0,
            dev_major: 0,
            dev_minor: 30,
            mnt_id: Some(0x20),
            dio_mem_align: None,
            dio_offset_align: None,
            subvol: Some(0x100),
            atomic_write_unit_min: None,
            atomic_write_unit_max: None,
            atomic_write_segments_max: None,
            dio_read_offset_align: None,
            atomic_write_unit_max_opt: None,
        }
    }

    #[test]
    fn flags_display() {
        assert_eq!(
            StatxFlags::AT_STATX_SYNC_AS_STAT.to_string(),
            "AT_STATX_SYNC_AS_STAT"
        );
        assert_eq!(
            StatxFlags::AT_STATX_FORCE_SYNC.to_string(),
            "AT_STATX_FORCE_SYNC"
        );
        assert_eq!(
            StatxFlags::AT_STATX_DONT_SYNC.to_string(),
            "AT_STATX_DONT_SYNC"
        );
        assert_eq!(
            StatxFlags(0x2000 | 0x4000).to_string(),
            "AT_STATX_FORCE_SYNC|AT_STATX_DONT_SYNC"
        );
        assert_eq!(
            StatxFlags(0x100).to_string(),
            "AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW"
        );
        assert_eq!(
            StatxFlags(0x900).to_string(),
            "AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW|AT_NO_AUTOMOUNT"
        );
        assert_eq!(
            StatxFlags(0x2100).to_string(),
            "AT_STATX_FORCE_SYNC|AT_SYMLINK_NOFOLLOW"
        );
        assert_eq!(
            StatxFlags(0x5000).to_string(),
            "AT_STATX_DONT_SYNC|AT_EMPTY_PATH"
        );
        assert_eq!(
            StatxFlags(0x10000).to_string(),
            "AT_STATX_SYNC_AS_STAT|0x10000"
        );
        assert_eq!(
            StatxFlags(0x10100).to_string(),
            "AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW|0x10000"
        );
    }

    #[test]
    fn flags_parse() {
        assert_eq!(
            StatxFlags::from_str("AT_STATX_SYNC_AS_STAT"),
            Ok(StatxFlags(0))
        );
        assert_eq!(
            StatxFlags::from_str("AT_STATX_FORCE_SYNC"),
            Ok(StatxFlags(0x2000))
        );
        assert_eq!(
            StatxFlags::from_str("AT_STATX_DONT_SYNC|AT_EMPTY_PATH"),
            Ok(StatxFlags(0x5000))
        );
        assert_eq!(
            StatxFlags::from_str("AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW|AT_NO_AUTOMOUNT"),
            Ok(StatxFlags(0x900))
        );
        assert_eq!(
            StatxFlags::from_str("AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW|0x10000"),
            Ok(StatxFlags(0x10100))
        );
        assert!(StatxFlags::from_str("AT_NOPE").is_err());
    }

    #[test]
    fn mask_display() {
        assert_eq!(StatxMask(0).to_string(), "0");
        assert_eq!(StatxMask::STATX_ALL.to_string(), "STATX_ALL");
        assert_eq!(
            StatxMask::STATX_BASIC_STATS.to_string(),
            "STATX_BASIC_STATS"
        );
        assert_eq!(
            StatxMask(0x00009fff).to_string(),
            "STATX_ALL|STATX_MNT_ID|STATX_SUBVOL"
        );
        assert_eq!(StatxMask(0x00001000).to_string(), "STATX_MNT_ID");
        assert_eq!(
            StatxMask(0x80000000).to_string(),
            "0x80000000 /* STATX_??? */"
        );
        assert_eq!(StatxMask(0x80001000).to_string(), "STATX_MNT_ID|0x80000000");
    }

    #[test]
    fn mask_parse() {
        assert_eq!(StatxMask::from_str("0"), Ok(StatxMask(0)));
        assert_eq!(StatxMask::from_str("STATX_ALL"), Ok(StatxMask::STATX_ALL));
        assert_eq!(
            StatxMask::from_str("STATX_ALL|STATX_MNT_ID|STATX_SUBVOL"),
            Ok(StatxMask(0x00009fff))
        );
        assert_eq!(
            StatxMask::from_str("STATX_MNT_ID|0x80000000"),
            Ok(StatxMask(0x80001000))
        );
        assert!(StatxMask::from_str("STATX_NOPE").is_err());
    }

    #[test]
    fn attr_flags_display() {
        assert_eq!(StatxAttrFlags(0).to_string(), "0");
        assert_eq!(
            StatxAttrFlags(0x14).to_string(),
            "STATX_ATTR_COMPRESSED|STATX_ATTR_IMMUTABLE"
        );
    }

    #[test]
    fn timestamp_display_and_parse() {
        let ts = StatxTimestamp::from_raw(1754766014, 986788270);
        assert_eq!(ts.to_string(), "{tv_sec=1754766014, tv_nsec=986788270}");
        let with_comment =
            "{tv_sec=1754766014, tv_nsec=986788270} /* 2025-08-09T20:00:14.986788270+0100 */";
        assert_eq!(StatxTimestamp::from_str(with_comment), Ok(ts));
    }

    #[test]
    fn buffer_display() {
        assert_eq!(regular().to_string(), REGULAR);
    }

    #[test]
    fn buffer_parse_with_comments() {
        let line = "{stx_mask=STATX_ALL|STATX_MNT_ID|STATX_SUBVOL, stx_blksize=4096, \
            stx_attributes=0, stx_nlink=1, stx_uid=0, stx_gid=0, stx_mode=S_IFREG|0644, \
            stx_ino=6997, stx_size=8, stx_blocks=8, \
            stx_attributes_mask=STATX_ATTR_COMPRESSED|STATX_ATTR_IMMUTABLE|STATX_ATTR_APPEND|\
            STATX_ATTR_NODUMP|STATX_ATTR_AUTOMOUNT|STATX_ATTR_MOUNT_ROOT|STATX_ATTR_DAX, \
            stx_atime={tv_sec=1754766014, tv_nsec=986788270} /* 2025-08-09T20:00:14.986788270+0100 */, \
            stx_btime={tv_sec=1754766014, tv_nsec=986788270} /* 2025-08-09T20:00:14.986788270+0100 */, \
            stx_ctime={tv_sec=1754766014, tv_nsec=986788270} /* 2025-08-09T20:00:14.986788270+0100 */, \
            stx_mtime={tv_sec=1545480610, tv_nsec=541614343} /* 2018-12-22T12:10:10.541614343+0000 */, \
            stx_rdev_major=0, stx_rdev_minor=0, stx_dev_major=0, stx_dev_minor=30, \
            stx_mnt_id=0x20, stx_subvol=0x100}";
        assert_eq!(StatxBuf::from_str(line), Ok(regular()));
    }

    #[test]
    fn buffer_parse_minimal() {
        let line = "{stx_mask=STATX_SIZE, stx_blksize=4096, stx_attributes=0, stx_size=8, \
            stx_attributes_mask=0, stx_rdev_major=0, stx_rdev_minor=0, stx_dev_major=0, \
            stx_dev_minor=30}";
        let buf = StatxBuf::from_str(line).unwrap();
        assert_eq!(buf.mask, StatxMask(0x200));
        assert_eq!(buf.size, Some(8));
        assert_eq!(buf.nlink, None);
        assert_eq!(buf.mode, None);
        assert_eq!(buf.to_string(), line);
    }

    #[test]
    fn buffer_parse_invalid() {
        assert!(StatxBuf::from_str("{}").is_err());
        assert!(StatxBuf::from_str("stx_mask=0").is_err());
        assert!(StatxBuf::from_str("{st_bogus=1}").is_err());
        assert!(
            StatxBuf::from_str(
                &REGULAR.replace("stx_mask=STATX_ALL|STATX_MNT_ID|STATX_SUBVOL, ", "")
            )
            .is_err()
        );
    }

    #[test]
    fn display_success() {
        let call = Statx::from_raw(
            DirFd::AT_FDCWD,
            "/etc/hostname",
            StatxFlags(0x900),
            StatxMask::STATX_ALL,
            Buffer::Decoded(regular()),
            0,
        );
        assert_eq!(
            call.to_string(),
            format!(
                "statx(AT_FDCWD, \"/etc/hostname\", \
                 AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW|AT_NO_AUTOMOUNT, STATX_ALL, \
                 {REGULAR}) = 0"
            )
        );
    }

    #[test]
    fn display_failure() {
        let call = Statx::from_raw(
            DirFd::AT_FDCWD,
            "/nonexistent",
            StatxFlags::AT_STATX_SYNC_AS_STAT,
            StatxMask::STATX_ALL,
            Buffer::Address(0x7f7b972f2130),
            -2,
        );
        assert_eq!(
            call.to_string(),
            "statx(AT_FDCWD, \"/nonexistent\", AT_STATX_SYNC_AS_STAT, STATX_ALL, \
             0x7f7b972f2130) = -1 ENOENT"
        );
    }

    #[test]
    fn parse_success() {
        let line = "statx(AT_FDCWD, \"/etc/hostname\", AT_STATX_SYNC_AS_STAT, STATX_ALL, \
             0x7f7b972f2130) = -1 ENOENT";
        let call = Statx::from_str(line).unwrap();
        assert_eq!(call.dirfd, DirFd::AT_FDCWD);
        assert_eq!(call.pathname.to_str().unwrap(), "/etc/hostname");
        assert_eq!(call.flags, StatxFlags(0));
        assert_eq!(call.mask, StatxMask::STATX_ALL);
        assert_eq!(call.buf, Buffer::Address(0x7f7b972f2130));
        assert_eq!(call.result, OkResult::Errno(Errno::ENOENT));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Statx::from_str("statx(AT_FDCWD, \"/a\") = 0").is_err());
        assert!(Statx::from_str("statx(AT_FDCWD, \"/a\", 0, 0) = 0").is_err());
        assert!(Statx::from_str("statx(AT_FDCWD, \"/a\", AT_NOPE, 0, NULL) = 0").is_err());
        assert!(Statx::from_str("stat(\"/a\", NULL) = 0").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "statx(AT_FDCWD, \"/etc/hostname\", AT_STATX_SYNC_AS_STAT, STATX_ALL, \
             0x7f7b972f2130) = -1 ENOENT",
            "statx(AT_FDCWD, \"/f\", AT_STATX_SYNC_AS_STAT, STATX_ALL, NULL) = -1 ENOENT",
            "statx(AT_FDCWD, \"/f\", AT_STATX_FORCE_SYNC, 0, NULL) = -1 EINVAL",
            "statx(AT_FDCWD, \"/f\", AT_STATX_DONT_SYNC|AT_EMPTY_PATH, STATX_SIZE, NULL) = \
             -1 EINVAL",
            "statx(AT_FDCWD, \"/f\", AT_STATX_SYNC_AS_STAT|AT_SYMLINK_NOFOLLOW|AT_NO_AUTOMOUNT, \
             STATX_ALL, NULL) = -1 ENOENT",
            "statx(AT_FDCWD, \"/f\", AT_STATX_SYNC_AS_STAT|0x10000, 0, NULL) = -1 EINVAL",
            "statx(AT_FDCWD, \"/f\", AT_STATX_SYNC_AS_STAT, 0x80000000 /* STATX_??? */, NULL) = \
             -1 EINVAL",
            "statx(AT_FDCWD, \"/f\", AT_STATX_SYNC_AS_STAT, STATX_MNT_ID|0x80000000, NULL) = \
             -1 EINVAL",
        ] {
            let syscall = Statx::from_str(line).unwrap();
            assert_eq!(syscall.to_string(), line);
        }
    }
}
