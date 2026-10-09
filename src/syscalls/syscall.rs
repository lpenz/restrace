// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`Syscall`] enum: every supported system call record.

use super::chdir::Chdir;
use super::clone::Clone;
use super::clone3::Clone3;
use super::close::Close;
use super::creat::Creat;
use super::execve::Execve;
use super::execveat::Execveat;
use super::fchdir::Fchdir;
use super::fork::Fork;
use super::generic::Generic;
use super::mkdir::Mkdir;
use super::mkdirat::Mkdirat;
use super::open::Open;
use super::openat::Openat;
use super::parse::ParseError;
use super::rename::Rename;
use super::renameat::Renameat;
use super::renameat2::Renameat2;
use super::rmdir::Rmdir;
use super::vfork::Vfork;
use std::fmt;
use std::str::FromStr;

/// A system call record, as one of the supported call types or, as a
/// fallback, a generic [`Generic`] call that no dedicated type parses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Syscall {
    /// A `chdir(2)` call.
    Chdir(Chdir),
    /// A `clone(2)` call.
    Clone(Clone),
    /// A `clone3(2)` call.
    Clone3(Clone3),
    /// A `close(2)` call.
    Close(Close),
    /// A `creat(2)` call.
    Creat(Creat),
    /// An `execve(2)` call.
    Execve(Execve),
    /// An `execveat(2)` call.
    Execveat(Execveat),
    /// An `fchdir(2)` call.
    Fchdir(Fchdir),
    /// A `fork(2)` call.
    Fork(Fork),
    /// A `mkdir(2)` call.
    Mkdir(Mkdir),
    /// A `mkdirat(2)` call.
    Mkdirat(Mkdirat),
    /// An `open(2)` call.
    Open(Open),
    /// An `openat(2)` call.
    Openat(Openat),
    /// A `rename(2)` call.
    Rename(Rename),
    /// A `renameat(2)` call.
    Renameat(Renameat),
    /// A `renameat2(2)` call.
    Renameat2(Renameat2),
    /// An `rmdir(2)` call.
    Rmdir(Rmdir),
    /// A `vfork(2)` call.
    Vfork(Vfork),
    /// A call that no dedicated type parses.
    Generic(Generic),
}

impl fmt::Display for Syscall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chdir(syscall) => syscall.fmt(f),
            Self::Clone(syscall) => syscall.fmt(f),
            Self::Clone3(syscall) => syscall.fmt(f),
            Self::Close(syscall) => syscall.fmt(f),
            Self::Creat(syscall) => syscall.fmt(f),
            Self::Execve(syscall) => syscall.fmt(f),
            Self::Execveat(syscall) => syscall.fmt(f),
            Self::Fchdir(syscall) => syscall.fmt(f),
            Self::Fork(syscall) => syscall.fmt(f),
            Self::Mkdir(syscall) => syscall.fmt(f),
            Self::Mkdirat(syscall) => syscall.fmt(f),
            Self::Open(syscall) => syscall.fmt(f),
            Self::Openat(syscall) => syscall.fmt(f),
            Self::Rename(syscall) => syscall.fmt(f),
            Self::Renameat(syscall) => syscall.fmt(f),
            Self::Renameat2(syscall) => syscall.fmt(f),
            Self::Rmdir(syscall) => syscall.fmt(f),
            Self::Vfork(syscall) => syscall.fmt(f),
            Self::Generic(syscall) => syscall.fmt(f),
        }
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it, dispatching to the
/// matching call type; calls that no type parses fall back to [`Generic`].
impl FromStr for Syscall {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        if let Ok(syscall) = Chdir::from_str(s) {
            return Ok(Self::Chdir(syscall));
        }
        if let Ok(syscall) = Clone::from_str(s) {
            return Ok(Self::Clone(syscall));
        }
        if let Ok(syscall) = Clone3::from_str(s) {
            return Ok(Self::Clone3(syscall));
        }
        if let Ok(syscall) = Close::from_str(s) {
            return Ok(Self::Close(syscall));
        }
        if let Ok(syscall) = Creat::from_str(s) {
            return Ok(Self::Creat(syscall));
        }
        if let Ok(syscall) = Execve::from_str(s) {
            return Ok(Self::Execve(syscall));
        }
        if let Ok(syscall) = Execveat::from_str(s) {
            return Ok(Self::Execveat(syscall));
        }
        if let Ok(syscall) = Fchdir::from_str(s) {
            return Ok(Self::Fchdir(syscall));
        }
        if let Ok(syscall) = Fork::from_str(s) {
            return Ok(Self::Fork(syscall));
        }
        if let Ok(syscall) = Mkdir::from_str(s) {
            return Ok(Self::Mkdir(syscall));
        }
        if let Ok(syscall) = Mkdirat::from_str(s) {
            return Ok(Self::Mkdirat(syscall));
        }
        if let Ok(syscall) = Open::from_str(s) {
            return Ok(Self::Open(syscall));
        }
        if let Ok(syscall) = Openat::from_str(s) {
            return Ok(Self::Openat(syscall));
        }
        if let Ok(syscall) = Rename::from_str(s) {
            return Ok(Self::Rename(syscall));
        }
        if let Ok(syscall) = Renameat::from_str(s) {
            return Ok(Self::Renameat(syscall));
        }
        if let Ok(syscall) = Renameat2::from_str(s) {
            return Ok(Self::Renameat2(syscall));
        }
        if let Ok(syscall) = Rmdir::from_str(s) {
            return Ok(Self::Rmdir(syscall));
        }
        if let Ok(syscall) = Vfork::from_str(s) {
            return Ok(Self::Vfork(syscall));
        }
        Generic::from_str(s).map(Self::Generic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_variants() {
        let is_chdir: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Chdir(_));
        let is_clone: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Clone(_));
        let is_clone3: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Clone3(_));
        let is_close: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Close(_));
        let is_creat: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Creat(_));
        let is_execve: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Execve(_));
        let is_execveat: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Execveat(_));
        let is_fchdir: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Fchdir(_));
        let is_fork: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Fork(_));
        let is_mkdir: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Mkdir(_));
        let is_mkdirat: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Mkdirat(_));
        let is_open: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Open(_));
        let is_openat: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Openat(_));
        let is_rename: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Rename(_));
        let is_renameat: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Renameat(_));
        let is_renameat2: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Renameat2(_));
        let is_rmdir: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Rmdir(_));
        let is_vfork: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Vfork(_));
        let is_generic: fn(&Syscall) -> bool = |s| matches!(s, Syscall::Generic(_));
        for (line, variant) in [
            ("chdir(\"/tmp\") = 0", is_chdir),
            (
                "clone(child_stack=NULL, flags=CLONE_VM|SIGCHLD) = 1",
                is_clone,
            ),
            (
                "clone3({flags=CLONE_VM, exit_signal=SIGCHLD}, 88) = 1",
                is_clone3,
            ),
            ("close(0) = 0", is_close),
            ("creat(\"/tmp/file\", 0644) = 3", is_creat),
            ("execve(\"/bin/echo\", [\"echo\"], []) = 0", is_execve),
            (
                "execveat(AT_FDCWD, \"prog\", [\"prog\"], [], 0) = 0",
                is_execveat,
            ),
            ("fchdir(0) = 0", is_fchdir),
            ("fork() = 123", is_fork),
            ("mkdir(\"/tmp/dir\", 0755) = 0", is_mkdir),
            ("mkdirat(AT_FDCWD, \"/tmp/dir\", 0755) = 0", is_mkdirat),
            ("open(\"/tmp\", O_RDONLY) = 3", is_open),
            ("openat(4, \"file\", O_RDONLY) = 3", is_openat),
            ("rename(\"/a\", \"/b\") = 0", is_rename),
            (
                "renameat(AT_FDCWD, \"a\", AT_FDCWD, \"b\") = 0",
                is_renameat,
            ),
            (
                "renameat2(AT_FDCWD, \"a\", AT_FDCWD, \"b\", 0) = 0",
                is_renameat2,
            ),
            ("rmdir(\"/tmp/dir\") = 0", is_rmdir),
            ("vfork() = 123", is_vfork),
            ("spill(0x1, 0x2) = 0", is_generic),
        ] {
            let syscall = Syscall::from_str(line).unwrap();
            assert!(variant(&syscall), "line {line:?} parsed wrongly");
        }
    }

    #[test]
    fn generic_fallback_on_invalid_known() {
        let syscall = Syscall::from_str("execve(\"/bin/echo\", [\"echo\", 0x1], []) = 0").unwrap();
        assert!(matches!(syscall, Syscall::Generic(_)));
    }

    #[test]
    fn parse_invalid() {
        assert!(Syscall::from_str("").is_err());
        assert!(Syscall::from_str("no syscall here").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "chdir(\"/tmp\") = 0",
            "close(3) = -1 EBADF",
            "open(\"/tmp/file\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3",
            "execve(\"/bin/echo\", [\"echo\", \"hello\"], [\"PATH=/bin\"]) = 0",
            "clone3({flags=CLONE_VM, exit_signal=SIGCHLD}, 88) = -1 EPERM",
            "spill(0x1000, -7, \"a, b\") = 0",
        ] {
            let syscall = Syscall::from_str(line).unwrap();
            assert_eq!(syscall.to_string(), line);
        }
    }
}
