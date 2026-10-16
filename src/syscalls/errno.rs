// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! `errno` values, as returned by failed system calls.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Names of the known `errno` constants, with their values.
///
/// This is the table `strace` uses on the generic Linux architecture (which
/// includes x86-64), so that the names parsed and printed here match the ones
/// `strace` emits.
const NAMES: &[(i32, &str)] = &[
    (1, "EPERM"),
    (2, "ENOENT"),
    (3, "ESRCH"),
    (4, "EINTR"),
    (5, "EIO"),
    (6, "ENXIO"),
    (7, "E2BIG"),
    (8, "ENOEXEC"),
    (9, "EBADF"),
    (10, "ECHILD"),
    (11, "EAGAIN"),
    (12, "ENOMEM"),
    (13, "EACCES"),
    (14, "EFAULT"),
    (15, "ENOTBLK"),
    (16, "EBUSY"),
    (17, "EEXIST"),
    (18, "EXDEV"),
    (19, "ENODEV"),
    (20, "ENOTDIR"),
    (21, "EISDIR"),
    (22, "EINVAL"),
    (23, "ENFILE"),
    (24, "EMFILE"),
    (25, "ENOTTY"),
    (26, "ETXTBSY"),
    (27, "EFBIG"),
    (28, "ENOSPC"),
    (29, "ESPIPE"),
    (30, "EROFS"),
    (31, "EMLINK"),
    (32, "EPIPE"),
    (33, "EDOM"),
    (34, "ERANGE"),
    (35, "EDEADLK"),
    (36, "ENAMETOOLONG"),
    (37, "ENOLCK"),
    (38, "ENOSYS"),
    (39, "ENOTEMPTY"),
    (40, "ELOOP"),
    (42, "ENOMSG"),
    (43, "EIDRM"),
    (44, "ECHRNG"),
    (45, "EL2NSYNC"),
    (46, "EL3HLT"),
    (47, "EL3RST"),
    (48, "ELNRNG"),
    (49, "EUNATCH"),
    (50, "ENOCSI"),
    (51, "EL2HLT"),
    (52, "EBADE"),
    (53, "EBADR"),
    (54, "EXFULL"),
    (55, "ENOANO"),
    (56, "EBADRQC"),
    (57, "EBADSLT"),
    (59, "EBFONT"),
    (60, "ENOSTR"),
    (61, "ENODATA"),
    (62, "ETIME"),
    (63, "ENOSR"),
    (64, "ENONET"),
    (65, "ENOPKG"),
    (66, "EREMOTE"),
    (67, "ENOLINK"),
    (68, "EADV"),
    (69, "ESRMNT"),
    (70, "ECOMM"),
    (71, "EPROTO"),
    (72, "EMULTIHOP"),
    (73, "EDOTDOT"),
    (74, "EBADMSG"),
    (75, "EOVERFLOW"),
    (76, "ENOTUNIQ"),
    (77, "EBADFD"),
    (78, "EREMCHG"),
    (79, "ELIBACC"),
    (80, "ELIBBAD"),
    (81, "ELIBSCN"),
    (82, "ELIBMAX"),
    (83, "ELIBEXEC"),
    (84, "EILSEQ"),
    (85, "ERESTART"),
    (86, "ESTRPIPE"),
    (87, "EUSERS"),
    (88, "ENOTSOCK"),
    (89, "EDESTADDRREQ"),
    (90, "EMSGSIZE"),
    (91, "EPROTOTYPE"),
    (92, "ENOPROTOOPT"),
    (93, "EPROTONOSUPPORT"),
    (94, "ESOCKTNOSUPPORT"),
    (95, "EOPNOTSUPP"),
    (96, "EPFNOSUPPORT"),
    (97, "EAFNOSUPPORT"),
    (98, "EADDRINUSE"),
    (99, "EADDRNOTAVAIL"),
    (100, "ENETDOWN"),
    (101, "ENETUNREACH"),
    (102, "ENETRESET"),
    (103, "ECONNABORTED"),
    (104, "ECONNRESET"),
    (105, "ENOBUFS"),
    (106, "EISCONN"),
    (107, "ENOTCONN"),
    (108, "ESHUTDOWN"),
    (109, "ETOOMANYREFS"),
    (110, "ETIMEDOUT"),
    (111, "ECONNREFUSED"),
    (112, "EHOSTDOWN"),
    (113, "EHOSTUNREACH"),
    (114, "EALREADY"),
    (115, "EINPROGRESS"),
    (116, "ESTALE"),
    (117, "EUCLEAN"),
    (118, "ENOTNAM"),
    (119, "ENAVAIL"),
    (120, "EISNAM"),
    (121, "EREMOTEIO"),
    (122, "EDQUOT"),
    (123, "ENOMEDIUM"),
    (124, "EMEDIUMTYPE"),
    (125, "ECANCELED"),
    (126, "ENOKEY"),
    (127, "EKEYEXPIRED"),
    (128, "EKEYREVOKED"),
    (129, "EKEYREJECTED"),
    (130, "EOWNERDEAD"),
    (131, "ENOTRECOVERABLE"),
    (132, "ERFKILL"),
    (133, "EHWPOISON"),
    (512, "ERESTARTSYS"),
    (513, "ERESTARTNOINTR"),
    (514, "ERESTARTNOHAND"),
    (515, "ENOIOCTLCMD"),
    (516, "ERESTART_RESTARTBLOCK"),
    (517, "EPROBE_DEFER"),
    (518, "EOPENSTALE"),
    (521, "EBADHANDLE"),
    (522, "ENOTSYNC"),
    (523, "EBADCOOKIE"),
    (524, "ENOTSUPP"),
    (525, "ETOOSMALL"),
    (526, "ESERVERFAULT"),
    (527, "EBADTYPE"),
    (528, "EJUKEBOX"),
    (529, "EIOCBQUEUED"),
    (530, "ERECALLCONFLICT"),
];

/// An `errno` value.
///
/// System calls report errors as a negated errno in their return value;
/// [`Errno::from_raw`] converts such a return value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Errno(pub i32);

impl Errno {
    /// Operation not permitted.
    pub const EPERM: Self = Self(1);
    /// No such file or directory.
    pub const ENOENT: Self = Self(2);
    /// Interrupted system call.
    pub const EINTR: Self = Self(4);
    /// I/O error.
    pub const EIO: Self = Self(5);
    /// Bad file descriptor.
    pub const EBADF: Self = Self(9);
    /// Permission denied.
    pub const EACCES: Self = Self(13);
    /// Bad address.
    pub const EFAULT: Self = Self(14);
    /// File exists.
    pub const EEXIST: Self = Self(17);
    /// Cross-device link.
    pub const EXDEV: Self = Self(18);
    /// Not a directory.
    pub const ENOTDIR: Self = Self(20);
    /// Is a directory.
    pub const EISDIR: Self = Self(21);
    /// Invalid argument.
    pub const EINVAL: Self = Self(22);
    /// File table overflow.
    pub const ENFILE: Self = Self(23);
    /// Too many open files.
    pub const EMFILE: Self = Self(24);
    /// No space left on device.
    pub const ENOSPC: Self = Self(28);
    /// Read-only file system.
    pub const EROFS: Self = Self(30);
    /// File name too long.
    pub const ENAMETOOLONG: Self = Self(36);
    /// Resource temporarily unavailable.
    pub const EAGAIN: Self = Self(11);
    /// Out of memory.
    pub const ENOMEM: Self = Self(12);
    /// Broken pipe.
    pub const EPIPE: Self = Self(32);
    /// Function not implemented.
    pub const ENOSYS: Self = Self(38);

    /// Converts a raw, negated system call return value into an [`Errno`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// magnitudes above 4095 are clamped to 4095.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        Self(raw.unsigned_abs().min(4095) as i32)
    }

    /// Returns the symbolic name of this errno, if known.
    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        NAMES
            .iter()
            .find(|&&(code, _)| code == self.0)
            .map(|&(_, name)| name)
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "{}", self.0),
        }
    }
}

/// Parses an errno from its symbolic name (`ENOENT`) or its decimal value
/// (`2`), as [`Display`](fmt::Display) writes them.  A leading `-` is
/// accepted and dropped, so that raw `-errno` return values parse too.
impl FromStr for Errno {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if let Some(&(code, _)) = NAMES.iter().find(|&&(_, name)| name == s) {
            return Ok(Self(code));
        }
        s.strip_prefix('-')
            .unwrap_or(s)
            .parse::<i32>()
            .map(Self)
            .map_err(|_| ParseError::new("errno", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_known() {
        assert_eq!(Errno::ENOENT.to_string(), "ENOENT");
        assert_eq!(Errno(2).to_string(), "ENOENT");
    }

    #[test]
    fn display_unknown() {
        assert_eq!(Errno(9999).to_string(), "9999");
    }

    #[test]
    fn from_raw() {
        assert_eq!(Errno::from_raw(-2), Errno::ENOENT);
        assert_eq!(Errno::from_raw(-4095), Errno(4095));
        assert_eq!(Errno::from_raw(-4096), Errno(4095));
    }

    #[test]
    fn name() {
        assert_eq!(Errno::ENOENT.name(), Some("ENOENT"));
        assert_eq!(Errno(3).name(), Some("ESRCH"));
        assert_eq!(Errno(9999).name(), None);
    }

    #[test]
    fn parse_name() {
        assert_eq!(Errno::from_str("ENOENT"), Ok(Errno::ENOENT));
        assert_eq!(Errno::from_str("ENOSYS"), Ok(Errno::ENOSYS));
        assert_eq!(Errno::from_str("EFAULT"), Ok(Errno::EFAULT));
        assert_eq!(Errno::from_str("EXDEV"), Ok(Errno::EXDEV));
        assert_eq!(Errno::from_str("ERESTARTSYS"), Ok(Errno(512)));
        assert_eq!(Errno::from_str("EHWPOISON"), Ok(Errno(133)));
    }

    #[test]
    fn display_coverage() {
        assert_eq!(Errno::EFAULT.to_string(), "EFAULT");
        assert_eq!(Errno::EXDEV.to_string(), "EXDEV");
        assert_eq!(Errno(39).to_string(), "ENOTEMPTY");
        assert_eq!(Errno(40).to_string(), "ELOOP");
        assert_eq!(Errno(512).to_string(), "ERESTARTSYS");
    }

    #[test]
    fn parse_number() {
        assert_eq!(Errno::from_str("2"), Ok(Errno::ENOENT));
        assert_eq!(Errno::from_str("-2"), Ok(Errno::ENOENT));
        assert_eq!(Errno::from_str("9999"), Ok(Errno(9999)));
    }

    #[test]
    fn parse_invalid() {
        assert_eq!(
            Errno::from_str("ENOSUCH"),
            Err(ParseError::new("errno", "ENOSUCH"))
        );
        assert!(Errno::from_str("").is_err());
        assert!(Errno::from_str("2x").is_err());
    }

    #[test]
    fn round_trip() {
        for &(code, name) in NAMES {
            let errno = Errno(code);
            assert_eq!(errno.to_string(), name);
            assert_eq!(Errno::from_str(name), Ok(errno));
        }
        assert_eq!(Errno::from_str(&Errno(9999).to_string()), Ok(Errno(9999)));
    }
}
