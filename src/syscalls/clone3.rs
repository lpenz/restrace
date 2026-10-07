// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`clone3(2)`] system call.
//!
//! [`clone3(2)`]: https://man7.org/linux/man-pages/man2/clone3.2.html

use super::clone::{CloneFlags, parse_addr, signal_from_str, signal_name};
use super::fork::ForkResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// The `clone_args` structure passed to `clone3(2)`.
///
/// ```text
/// struct clone_args {
///     u64 flags;
///     u64 pidfd;
///     u64 child_tid;
///     u64 parent_tid;
///     u64 exit_signal;
///     u64 stack;
///     u64 stack_size;
///     u64 tls;
///     u64 set_tid;
///     u64 set_tid_size;
/// };
/// ```
///
/// Fields that are zero are not shown when tracing, as strace does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clone3Args {
    /// The `flags` field, whose low byte is the exit signal.
    pub flags: CloneFlags,
    /// Location for a `pidfd` referring to the child, from `CLONE_PIDFD`.
    pub pidfd: Option<u64>,
    /// Location for the child TID in the child, from `CLONE_CHILD_CLEARTID`.
    pub child_tid: Option<u64>,
    /// Location for the child TID in the parent, from `CLONE_PARENT_SETTID`.
    pub parent_tid: Option<u64>,
    /// The signal sent to the parent when the child exits.
    pub exit_signal: Option<u64>,
    /// The stack the child runs on.
    pub stack: Option<u64>,
    /// The size of the stack above.
    pub stack_size: Option<u64>,
    /// The thread-local-storage pointer, from `CLONE_SETTLS`.
    pub tls: Option<u64>,
    /// Pointer to the array of TIDs the child is placed into, from
    /// `CLONE_NEWPID`.
    pub set_tid: Option<u64>,
    /// The length of the `set_tid` array.
    pub set_tid_size: Option<u64>,
}

impl Clone3Args {
    /// Builds an argument struct with only `flags` set.
    #[must_use]
    pub fn new(flags: CloneFlags) -> Self {
        Self {
            flags,
            pidfd: None,
            child_tid: None,
            parent_tid: None,
            exit_signal: None,
            stack: None,
            stack_size: None,
            tls: None,
            set_tid: None,
            set_tid_size: None,
        }
    }
}

impl fmt::Display for Clone3Args {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "flags={}", self.flags)?;
        if let Some(pidfd) = self.pidfd {
            write!(f, ", pidfd=0x{pidfd:x}")?;
        }
        if let Some(child_tid) = self.child_tid {
            write!(f, ", child_tid=0x{child_tid:x}")?;
        }
        if let Some(parent_tid) = self.parent_tid {
            write!(f, ", parent_tid=0x{parent_tid:x}")?;
        }
        if let Some(exit_signal) = self.exit_signal {
            write!(f, ", exit_signal=")?;
            match signal_name(exit_signal as u32) {
                Some(name) => f.write_str(name)?,
                None => write!(f, "{exit_signal}")?,
            }
        }
        if let Some(stack) = self.stack {
            write!(f, ", stack=0x{stack:x}")?;
        }
        if let Some(stack_size) = self.stack_size {
            write!(f, ", stack_size=0x{stack_size:x}")?;
        }
        if let Some(tls) = self.tls {
            write!(f, ", tls=0x{tls:x}")?;
        }
        if let Some(set_tid) = self.set_tid {
            write!(f, ", set_tid=0x{set_tid:x}")?;
        }
        if let Some(set_tid_size) = self.set_tid_size {
            write!(f, ", set_tid_size=0x{set_tid_size:x}")?;
        }
        Ok(())
    }
}

/// Arguments and return value of the `clone3(2)` system call.
///
/// ```text
/// int clone3(struct clone_args *cl_args, size_t size);
/// ```
///
/// Strace prints the argument struct between braces, only the non-zero
/// fields, followed by the struct size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clone3 {
    /// The `clone_args` structure, the first argument.
    pub args: Clone3Args,
    /// Size of the `clone_args` structure, the second argument.
    pub size: u64,
    /// Return value: child PID in the parent, 0 in the child, or -1 on error.
    pub result: ForkResult,
}

impl Clone3 {
    /// Builds a `clone3(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the child
    /// PID (or 0 in the child), or `-errno` on failure.
    #[must_use]
    pub fn from_raw(args: Clone3Args, size: u64, raw_result: i64) -> Self {
        Self {
            args,
            size,
            result: ForkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Clone3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "clone3({{{}}}, {}) = {}",
            self.args, self.size, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `clone3({flags=CLONE_VM|SIGCHLD, stack=0x1000, stack_size=0x1000}, 88) = pid`.
impl FromStr for Clone3 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("clone3 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("clone3({").ok_or_else(syntax)?;
        let (body, size) = args.rsplit_once("}, ").ok_or_else(syntax)?;
        let mut args = Clone3Args::new(CloneFlags::default());
        let mut flags_seen = false;
        for part in parse::split_args(body) {
            let (key, value) = part.split_once('=').ok_or_else(syntax)?;
            match key.trim() {
                "flags" => {
                    args.flags = CloneFlags::from_str(value)?;
                    flags_seen = true;
                }
                "pidfd" => args.pidfd = Some(parse_addr(value, "pidfd")?),
                "child_tid" => args.child_tid = Some(parse_addr(value, "child_tid")?),
                "parent_tid" => args.parent_tid = Some(parse_addr(value, "parent_tid")?),
                "exit_signal" => args.exit_signal = Some(u64::from(signal_from_str(value)?)),
                "stack" => args.stack = Some(parse_addr(value, "stack")?),
                "stack_size" => args.stack_size = Some(parse_addr(value, "stack_size")?),
                "tls" => args.tls = Some(parse_addr(value, "tls")?),
                "set_tid" => args.set_tid = Some(parse_addr(value, "set_tid")?),
                "set_tid_size" => args.set_tid_size = Some(parse_addr(value, "set_tid_size")?),
                _ => return Err(ParseError::new("clone3 argument", part)),
            }
        }
        if !flags_seen {
            return Err(syntax());
        }
        Ok(Self {
            args,
            size: parse_addr(size, "size")?,
            result: ForkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_success() {
        let clone3 = Clone3::from_raw(
            Clone3Args {
                exit_signal: Some(17),
                stack: Some(0x7fff1234),
                stack_size: Some(0x2800),
                ..Clone3Args::new(CloneFlags::CLONE_VM | CloneFlags::CLONE_VFORK)
            },
            88,
            12345,
        );
        assert_eq!(
            clone3.to_string(),
            "clone3({flags=CLONE_VM|CLONE_VFORK, exit_signal=SIGCHLD, stack=0x7fff1234, stack_size=0x2800}, 88) = 12345"
        );
        assert_eq!(clone3.result, ForkResult::Pid(12345));
    }

    #[test]
    fn display_set_tid() {
        let clone3 = Clone3::from_raw(
            Clone3Args {
                set_tid: Some(0x7fff),
                set_tid_size: Some(1),
                ..Clone3Args::new(CloneFlags::CLONE_NEWPID)
            },
            88,
            0,
        );
        assert_eq!(
            clone3.to_string(),
            "clone3({flags=CLONE_NEWPID, set_tid=0x7fff, set_tid_size=0x1}, 88) = 0"
        );
    }

    #[test]
    fn parse_success() {
        let clone3 = Clone3::from_str(
            "clone3({flags=CLONE_VM|CLONE_THREAD, exit_signal=SIGCHLD, stack=0x1000, stack_size=0x1000, tls=0x2000}, 88) = 42",
        )
        .unwrap();
        assert_eq!(
            clone3.args.flags,
            CloneFlags::CLONE_VM | CloneFlags::CLONE_THREAD
        );
        assert_eq!(clone3.args.exit_signal, Some(17));
        assert_eq!(clone3.size, 88);
        assert_eq!(clone3.result, ForkResult::Pid(42));
    }

    #[test]
    fn parse_failure() {
        let clone3 = Clone3::from_str(
            "clone3({flags=0, exit_signal=17, parent_tid=0x1000}, 88) = -1 EINVAL",
        )
        .unwrap();
        assert_eq!(clone3.args.flags, CloneFlags::default());
        assert_eq!(clone3.args.parent_tid, Some(0x1000));
        assert_eq!(
            clone3.result,
            ForkResult::Errno(crate::syscalls::errno::Errno::EINVAL)
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(Clone3::from_str("clone3({flags=CLONE_VM}, 88) 1").is_err());
        assert!(Clone3::from_str("clone3({}, 88) = 1").is_err());
        assert!(Clone3::from_str("clone3({stack=0x1000}, 88) = 1").is_err());
        assert!(Clone3::from_str("clone3({flags=BOGUS}, 88) = 1").is_err());
        assert!(Clone3::from_str("clone3({flags=CLONE_VM}, overwritten) = 1").is_err());
        assert!(Clone3::from_str("clone(child_stack=NULL, flags=CLONE_VM) = 1").is_err());
        assert!(Clone3::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "clone3({flags=CLONE_VM|CLONE_THREAD, exit_signal=SIGCHLD}, 88) = 3962",
            "clone3({flags=CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD|CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_CLEARTID, exit_signal=SIGCHLD, stack=0x7f4a1c000000, stack_size=0x8000, tls=0x7f4a1c4c1000}, 88) = 3962",
            "clone3({flags=0, exit_signal=40}, 88) = -1 EINVAL",
            "clone3({flags=CLONE_NEWNS, exit_signal=SIGCHLD}, 88) = -1 EPERM",
        ] {
            assert_eq!(Clone3::from_str(line).unwrap().to_string(), line);
        }
    }
}
