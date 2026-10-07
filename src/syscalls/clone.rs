// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`clone(2)`] system call.
//!
//! [`clone(2)`]: https://man7.org/linux/man-pages/man2/clone.2.html

use super::fork::ForkResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::str::FromStr;

/// Names of the signals a `clone`-family call can use as its exit signal.
///
/// The numeric values are the Linux ones.
pub(crate) const SIGNALS: &[(u32, &str)] = &[
    (1, "SIGHUP"),
    (2, "SIGINT"),
    (3, "SIGQUIT"),
    (4, "SIGILL"),
    (5, "SIGTRAP"),
    (6, "SIGABRT"),
    (7, "SIGBUS"),
    (8, "SIGFPE"),
    (9, "SIGKILL"),
    (10, "SIGUSR1"),
    (11, "SIGSEGV"),
    (12, "SIGUSR2"),
    (13, "SIGPIPE"),
    (14, "SIGALRM"),
    (15, "SIGTERM"),
    (16, "SIGSTKFLT"),
    (17, "SIGCHLD"),
    (18, "SIGCONT"),
    (19, "SIGSTOP"),
    (20, "SIGTSTP"),
    (21, "SIGTTIN"),
    (22, "SIGTTOU"),
    (23, "SIGURG"),
    (24, "SIGXCPU"),
    (25, "SIGXFSZ"),
    (26, "SIGVTALRM"),
    (27, "SIGPROF"),
    (28, "SIGWINCH"),
    (29, "SIGIO"),
    (30, "SIGPWR"),
    (31, "SIGSYS"),
];

/// Returns the name of `signal`, if known.
pub(crate) fn signal_name(signal: u32) -> Option<&'static str> {
    SIGNALS
        .iter()
        .find(|&&(value, _)| value == signal)
        .map(|&(_, name)| name)
}

/// Parses a signal from its name (`SIGCHLD`) or its decimal value.
pub(crate) fn signal_from_str(s: &str) -> Result<u32, ParseError> {
    let s = s.trim();
    if let Some(&(value, _)) = SIGNALS.iter().find(|&&(_, name)| name == s) {
        return Ok(value);
    }
    s.parse::<u32>().map_err(|_| ParseError::new("signal", s))
}

/// Parses a `0x`-prefixed hexadecimal or decimal address.
pub(crate) fn parse_addr(token: &str, part: &'static str) -> Result<u64, ParseError> {
    let error = || ParseError::new(part, token);
    let token = token.trim();
    if let Some(hex) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|_| error())
    } else {
        token.parse::<u64>().map_err(|_| error())
    }
}

/// The `flags` argument of `clone(2)` and `clone3(2)`.
///
/// Values are the Linux ones.  The low byte is the exit signal, printed by
/// name (e.g. `SIGCHLD`) as strace does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct CloneFlags(pub u32);

impl CloneFlags {
    /// Mask of the bits selecting the signal sent to the parent when the
    /// child exits.
    pub const CSIGNAL: Self = Self(0x000000ff);
    /// Share the memory address space of the caller with the child.
    pub const CLONE_VM: Self = Self(0x00000100);
    /// Share the filesystem context with the child.
    pub const CLONE_FS: Self = Self(0x00000200);
    /// Share the file descriptor table with the child.
    pub const CLONE_FILES: Self = Self(0x00000400);
    /// Share the signal handler table with the child.
    pub const CLONE_SIGHAND: Self = Self(0x00000800);
    /// Return a `pidfd` referring to the child.
    pub const CLONE_PIDFD: Self = Self(0x00001000);
    /// Make the parent block until a `ptrace` event happens on the child.
    pub const CLONE_PTRACE: Self = Self(0x00002000);
    /// Free the parent memory until the child calls `execve` or exits.
    pub const CLONE_VFORK: Self = Self(0x00004000);
    /// Make the creating process the parent of the child.
    pub const CLONE_PARENT: Self = Self(0x00008000);
    /// Share the thread group with the child.
    pub const CLONE_THREAD: Self = Self(0x00010000);
    /// Create a new mount namespace.
    pub const CLONE_NEWNS: Self = Self(0x00020000);
    /// Share the System V semaphore undo list.
    pub const CLONE_SYSVSEM: Self = Self(0x00040000);
    /// Create the child in the same thread group, with `tls` set.
    pub const CLONE_SETTLS: Self = Self(0x00080000);
    /// Store the child TID in the parent's memory at `parent_tid`.
    pub const CLONE_PARENT_SETTID: Self = Self(0x00100000);
    /// Clear the child TID in the child's memory when the child exits.
    pub const CLONE_CHILD_CLEARTID: Self = Self(0x00200000);
    /// Deprecated: release the parent memory on exit.
    pub const CLONE_DETACHED: Self = Self(0x00400000);
    /// Continue tracing the child even if parent is not traced.
    pub const CLONE_UNTRACED: Self = Self(0x00800000);
    /// Store the child TID in the child's memory at `child_tid`.
    pub const CLONE_CHILD_SETTID: Self = Self(0x01000000);
    /// Create a new cgroup namespace.
    pub const CLONE_NEWCGROUP: Self = Self(0x02000000);
    /// Create a new UTS namespace.
    pub const CLONE_NEWUTS: Self = Self(0x04000000);
    /// Create a new IPC namespace.
    pub const CLONE_NEWIPC: Self = Self(0x08000000);
    /// Create a new user namespace.
    pub const CLONE_NEWUSER: Self = Self(0x10000000);
    /// Create a new PID namespace.
    pub const CLONE_NEWPID: Self = Self(0x20000000);
    /// Create a new network namespace.
    pub const CLONE_NEWNET: Self = Self(0x40000000);
    /// Share the I/O context with the child.
    pub const CLONE_IO: Self = Self(0x80000000);

    // Union of the exit-signal mask and every known flag, for
    // [`CloneFlags::unknown_bits`].
    const KNOWN_MASK: u32 = Self::CSIGNAL.0
        | Self::CLONE_VM.0
        | Self::CLONE_FS.0
        | Self::CLONE_FILES.0
        | Self::CLONE_SIGHAND.0
        | Self::CLONE_PIDFD.0
        | Self::CLONE_PTRACE.0
        | Self::CLONE_VFORK.0
        | Self::CLONE_PARENT.0
        | Self::CLONE_THREAD.0
        | Self::CLONE_NEWNS.0
        | Self::CLONE_SYSVSEM.0
        | Self::CLONE_SETTLS.0
        | Self::CLONE_PARENT_SETTID.0
        | Self::CLONE_CHILD_CLEARTID.0
        | Self::CLONE_DETACHED.0
        | Self::CLONE_UNTRACED.0
        | Self::CLONE_CHILD_SETTID.0
        | Self::CLONE_NEWCGROUP.0
        | Self::CLONE_NEWUTS.0
        | Self::CLONE_NEWIPC.0
        | Self::CLONE_NEWUSER.0
        | Self::CLONE_NEWPID.0
        | Self::CLONE_NEWNET.0
        | Self::CLONE_IO.0;

    /// Returns true if all bits of `flag` are set in these flags.
    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }

    /// Returns the exit-signal bits of these flags.
    #[must_use]
    pub const fn signal(self) -> u32 {
        self.0 & Self::CSIGNAL.0
    }

    /// Returns the bits of these flags that no known flag covers.
    #[must_use]
    pub const fn unknown_bits(self) -> u32 {
        self.0 & !Self::KNOWN_MASK
    }
}

impl BitOr for CloneFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for CloneFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for CloneFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for CloneFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for CloneFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

const CLONE_FLAGS: &[(CloneFlags, &str)] = &[
    (CloneFlags::CLONE_VM, "CLONE_VM"),
    (CloneFlags::CLONE_FS, "CLONE_FS"),
    (CloneFlags::CLONE_FILES, "CLONE_FILES"),
    (CloneFlags::CLONE_SIGHAND, "CLONE_SIGHAND"),
    (CloneFlags::CLONE_PIDFD, "CLONE_PIDFD"),
    (CloneFlags::CLONE_PTRACE, "CLONE_PTRACE"),
    (CloneFlags::CLONE_VFORK, "CLONE_VFORK"),
    (CloneFlags::CLONE_PARENT, "CLONE_PARENT"),
    (CloneFlags::CLONE_THREAD, "CLONE_THREAD"),
    (CloneFlags::CLONE_NEWNS, "CLONE_NEWNS"),
    (CloneFlags::CLONE_SYSVSEM, "CLONE_SYSVSEM"),
    (CloneFlags::CLONE_SETTLS, "CLONE_SETTLS"),
    (CloneFlags::CLONE_PARENT_SETTID, "CLONE_PARENT_SETTID"),
    (CloneFlags::CLONE_CHILD_CLEARTID, "CLONE_CHILD_CLEARTID"),
    (CloneFlags::CLONE_DETACHED, "CLONE_DETACHED"),
    (CloneFlags::CLONE_UNTRACED, "CLONE_UNTRACED"),
    (CloneFlags::CLONE_CHILD_SETTID, "CLONE_CHILD_SETTID"),
    (CloneFlags::CLONE_NEWCGROUP, "CLONE_NEWCGROUP"),
    (CloneFlags::CLONE_NEWUTS, "CLONE_NEWUTS"),
    (CloneFlags::CLONE_NEWIPC, "CLONE_NEWIPC"),
    (CloneFlags::CLONE_NEWUSER, "CLONE_NEWUSER"),
    (CloneFlags::CLONE_NEWPID, "CLONE_NEWPID"),
    (CloneFlags::CLONE_NEWNET, "CLONE_NEWNET"),
    (CloneFlags::CLONE_IO, "CLONE_IO"),
];

impl fmt::Display for CloneFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("0");
        }
        let mut first = true;
        let mut write_part = |part: &str| -> fmt::Result {
            if !first {
                f.write_str("|")?;
            }
            first = false;
            f.write_str(part)
        };
        for (flag, name) in CLONE_FLAGS {
            if self.contains(*flag) {
                write_part(name)?;
            }
        }
        let signal = self.signal();
        if signal != 0 {
            match signal_name(signal) {
                Some(name) => write_part(name)?,
                None => write_part(&signal.to_string())?,
            }
        }
        let unknown = self.unknown_bits();
        if unknown != 0 {
            write_part(&format!("0x{unknown:x}"))?;
        }
        Ok(())
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them: flag names joined
/// by `|`, the exit signal by name, plus `0x`-prefixed hexadecimal for bits
/// that have no name; `0` for no flags.
impl FromStr for CloneFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::new("flags", s));
        }
        if s == "0" {
            return Ok(Self::default());
        }
        let mut flags = Self::default();
        for token in s.split('|') {
            let token = token.trim();
            let named = CLONE_FLAGS
                .iter()
                .find(|&&(_, name)| name == token)
                .map(|&(flag, _)| flag);
            let flag = match named {
                Some(flag) => flag,
                None => match signal_from_str(token) {
                    Ok(value) => Self(value),
                    Err(_) => Self(parse_number(token, "flags")?),
                },
            };
            flags |= flag;
        }
        Ok(flags)
    }
}

/// Parses a number with an optional `0x` or `0o` prefix.
fn parse_number(token: &str, part: &'static str) -> Result<u32, ParseError> {
    let error = |_| ParseError::new(part, token);
    if let Some(hex) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).map_err(error)
    } else if let Some(oct) = token
        .strip_prefix("0o")
        .or_else(|| token.strip_prefix("0O"))
    {
        u32::from_str_radix(oct, 8).map_err(error)
    } else {
        token.parse::<u32>().map_err(error)
    }
}

/// Arguments and return value of the `clone(2)` system call.
///
/// ```text
/// long clone(unsigned long flags, void *child_stack, int *parent_tid,
///            int *child_tid, unsigned long tls);
/// ```
///
/// Strace prints the arguments by name; only the tid/tls pointers that the
/// corresponding `CLONE_*` flags request are present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clone {
    /// The `flags` argument, whose low byte is the exit signal.
    pub flags: CloneFlags,
    /// The stack the child runs on, or NULL; the `child_stack` argument.
    pub stack: Option<u64>,
    /// Where the child TID is stored in the parent, the `parent_tid`
    /// argument, present when `CLONE_PARENT_SETTID` is set.
    pub parent_tid: Option<u64>,
    /// The thread-local-storage pointer, the `tls` argument, present when
    /// `CLONE_SETTLS` is set.
    pub tls: Option<u64>,
    /// Where the child TID is stored in the child, the `child_tid` argument,
    /// present when `CLONE_CHILD_SETTID` or `CLONE_CHILD_CLEARTID` is set.
    pub child_tid: Option<u64>,
    /// Return value: child PID in the parent, 0 in the child, or -1 on error.
    pub result: ForkResult,
}

impl Clone {
    /// Builds a `clone(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the child
    /// PID (or 0 in the child), or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        flags: CloneFlags,
        stack: Option<u64>,
        parent_tid: Option<u64>,
        tls: Option<u64>,
        child_tid: Option<u64>,
        raw_result: i64,
    ) -> Self {
        Self {
            flags,
            stack,
            parent_tid,
            tls,
            child_tid,
            result: ForkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Clone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "clone(child_stack={}, flags={}",
            self.stack
                .map(|stack| format!("0x{stack:x}"))
                .as_deref()
                .unwrap_or("NULL"),
            self.flags
        )?;
        if let Some(parent_tid) = self.parent_tid {
            write!(f, ", parent_tid=0x{parent_tid:x}")?;
        }
        if let Some(tls) = self.tls {
            write!(f, ", tls=0x{tls:x}")?;
        }
        if let Some(child_tid) = self.child_tid {
            write!(f, ", child_tid=0x{child_tid:x}")?;
        }
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it, with the named
/// strace arguments: `clone(child_stack=NULL, flags=CLONE_VM|SIGCHLD) = pid`.
impl FromStr for Clone {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("clone call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("clone(").ok_or_else(syntax)?;
        let mut flags = None;
        let mut stack = None;
        let mut parent_tid = None;
        let mut tls = None;
        let mut child_tid = None;
        for part in parse::split_args(args) {
            let (key, value) = part.split_once('=').ok_or_else(syntax)?;
            match key.trim() {
                "flags" => flags = Some(CloneFlags::from_str(value)?),
                "child_stack" => {
                    stack = Some(match value.trim() {
                        "NULL" => None,
                        value => Some(parse_addr(value, "child_stack")?),
                    });
                }
                "parent_tid" => parent_tid = Some(parse_addr(value, "parent_tid")?),
                "tls" => tls = Some(parse_addr(value, "tls")?),
                "child_tid" => child_tid = Some(parse_addr(value, "child_tid")?),
                _ => return Err(ParseError::new("clone argument", part)),
            }
        }
        Ok(Self {
            flags: flags.ok_or_else(syntax)?,
            stack: stack.ok_or_else(syntax)?,
            parent_tid,
            tls,
            child_tid,
            result: ForkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_display_zero() {
        assert_eq!(CloneFlags::default().to_string(), "0");
    }

    #[test]
    fn flags_display_combination() {
        let flags = CloneFlags::CLONE_VM | CloneFlags::CLONE_VFORK | CloneFlags(17);
        assert_eq!(flags.to_string(), "CLONE_VM|CLONE_VFORK|SIGCHLD");
    }

    #[test]
    fn flags_display_signal_only() {
        assert_eq!(CloneFlags(17).to_string(), "SIGCHLD");
    }

    #[test]
    fn flags_display_hex() {
        assert_eq!(
            (CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_VM).to_string(),
            "CLONE_VM|CLONE_NEWNET"
        );
    }

    #[test]
    fn signal_and_contains() {
        assert_eq!((CloneFlags(15) | CloneFlags::CLONE_VM).signal(), 15);
        assert!((CloneFlags(15) | CloneFlags::CLONE_VM).contains(CloneFlags::CLONE_VM));
        assert!(!CloneFlags::CLONE_VM.contains(CloneFlags::CSIGNAL));
    }

    #[test]
    fn flags_parse() {
        assert_eq!(CloneFlags::from_str("0"), Ok(CloneFlags::default()));
        assert_eq!(
            CloneFlags::from_str("CLONE_VM|CLONE_VFORK|SIGCHLD"),
            Ok(CloneFlags::CLONE_VM | CloneFlags::CLONE_VFORK | CloneFlags(17))
        );
        assert_eq!(CloneFlags::from_str("0x100"), Ok(CloneFlags::CLONE_VM));
        assert_eq!(CloneFlags::from_str("SIGKILL"), Ok(CloneFlags(9)));
        assert!(CloneFlags::from_str("CLONE_BOGUS").is_err());
        assert!(CloneFlags::from_str("").is_err());
    }

    #[test]
    fn display_success() {
        let clone = Clone::from_raw(
            CloneFlags::CLONE_VM | CloneFlags::CLONE_VFORK | CloneFlags(17),
            None,
            None,
            None,
            None,
            12345,
        );
        assert_eq!(
            clone.to_string(),
            "clone(child_stack=NULL, flags=CLONE_VM|CLONE_VFORK|SIGCHLD) = 12345"
        );
        assert_eq!(clone.result, ForkResult::Pid(12345));
    }

    #[test]
    fn display_tid_and_tls() {
        let clone = Clone::from_raw(
            CloneFlags::CLONE_PARENT_SETTID
                | CloneFlags::CLONE_SETTLS
                | CloneFlags::CLONE_CHILD_SETTID
                | CloneFlags(17),
            Some(0x7ffc1234),
            Some(0x7ffc2ab0),
            Some(0x7ffc2bb0),
            Some(0x7ffc2ab0),
            0,
        );
        assert_eq!(
            clone.to_string(),
            "clone(child_stack=0x7ffc1234, flags=CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_SETTID|SIGCHLD, parent_tid=0x7ffc2ab0, tls=0x7ffc2bb0, child_tid=0x7ffc2ab0) = 0"
        );
    }

    #[test]
    fn parse_success() {
        let clone =
            Clone::from_str("clone(child_stack=NULL, flags=CLONE_VM|SIGCHLD) = 42").unwrap();
        assert_eq!(
            clone,
            Clone::from_raw(
                CloneFlags::CLONE_VM | CloneFlags(17),
                None,
                None,
                None,
                None,
                42
            )
        );
        assert!(clone.flags.contains(CloneFlags::CLONE_VM));
    }

    #[test]
    fn parse_failure() {
        let clone =
            Clone::from_str("clone(child_stack=0x7ffc, flags=CLONE_NEWNS, tls=0x1000) = -1 EPERM")
                .unwrap();
        assert_eq!(clone.stack, Some(0x7ffc));
        assert_eq!(clone.tls, Some(0x1000));
        assert_eq!(
            clone.result,
            ForkResult::Errno(crate::syscalls::errno::Errno::EPERM)
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(Clone::from_str("clone(child_stack=NULL, flags=CLONE_VM) 1").is_err());
        assert!(Clone::from_str("clone(child_stack=NULL) = 1").is_err());
        assert!(Clone::from_str("clone(child_stack=BOGUS, flags=CLONE_VM) = 1").is_err());
        assert!(Clone::from_str("clone(child_stack=NULL, flags=BOGUS) = 1").is_err());
        assert!(Clone::from_str("clone(child_stack=NULL, flags=CLONE_VM, bogus=1) = 1").is_err());
        assert!(Clone::from_str("clone3({flags=CLONE_VM}, 88) = 1").is_err());
        assert!(Clone::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "clone(child_stack=NULL, flags=CLONE_VM|CLONE_VFORK|SIGCHLD) = 12345",
            "clone(child_stack=0x7ffc1234, flags=CLONE_VM|CLONE_SIGHAND|CLONE_THREAD, parent_tid=0x1000, tls=0x2000, child_tid=0x1000) = 0",
            "clone(child_stack=NULL, flags=0) = -1 EAGAIN",
            "clone(child_stack=0x0, flags=CLONE_NEWPID) = -1 EPERM",
        ] {
            assert_eq!(Clone::from_str(line).unwrap().to_string(), line);
        }
    }
}
