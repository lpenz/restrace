// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`Action`] enum: a system call record or a process exit.

use crate::exit::Exit;
use crate::syscalls::{
    Access, Chdir, Clone, Clone3, Close, Creat, Dup, Dup2, Dup3, Execve, Execveat, Faccessat,
    Faccessat2, Fchdir, Fork, Fstat, Generic, Getdents, Getdents64, Link, Linkat, Lstat, Mkdir,
    Mkdirat, Newfstatat, Open, Openat, Openat2, ParseError, Pipe2, Readlink, Readlinkat, Rename,
    Renameat, Renameat2, Rmdir, Stat, Symlink, Symlinkat, Unlink, Unlinkat, Vfork,
};
use std::fmt;
use std::str::FromStr;

/// What a traced process did: a supported system call, the [`Generic`]
/// fallback for a call that no dedicated type parses, or the [`Exit`] that
/// terminated the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// An `access(2)` call.
    Access(Access),
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
    /// A `dup(2)` call.
    Dup(Dup),
    /// A `dup2(2)` call.
    Dup2(Dup2),
    /// A `dup3(2)` call.
    Dup3(Dup3),
    /// An `execve(2)` call.
    Execve(Execve),
    /// An `execveat(2)` call.
    Execveat(Execveat),
    /// The exit of the traced process.
    Exit(Exit),
    /// An `faccessat(2)` call.
    Faccessat(Faccessat),
    /// An `faccessat2(2)` call.
    Faccessat2(Faccessat2),
    /// An `fchdir(2)` call.
    Fchdir(Fchdir),
    /// A `fork(2)` call.
    Fork(Fork),
    /// An `fstat(2)` call.
    Fstat(Fstat),
    /// A `getdents(2)` call.
    Getdents(Getdents),
    /// A `getdents64(2)` call.
    Getdents64(Getdents64),
    /// A `link(2)` call.
    Link(Link),
    /// A `linkat(2)` call.
    Linkat(Linkat),
    /// An `lstat(2)` call.
    Lstat(Lstat),
    /// A `mkdir(2)` call.
    Mkdir(Mkdir),
    /// A `mkdirat(2)` call.
    Mkdirat(Mkdirat),
    /// A `newfstatat(2)` call.
    Newfstatat(Newfstatat),
    /// An `open(2)` call.
    Open(Open),
    /// An `openat(2)` call.
    Openat(Openat),
    /// An `openat2(2)` call.
    Openat2(Openat2),
    /// A `pipe2(2)` call.
    Pipe2(Pipe2),
    /// A `readlink(2)` call.
    Readlink(Readlink),
    /// A `readlinkat(2)` call.
    Readlinkat(Readlinkat),
    /// A `rename(2)` call.
    Rename(Rename),
    /// A `renameat(2)` call.
    Renameat(Renameat),
    /// A `renameat2(2)` call.
    Renameat2(Renameat2),
    /// An `rmdir(2)` call.
    Rmdir(Rmdir),
    /// A `stat(2)` call.
    Stat(Stat),
    /// A `symlink(2)` call.
    Symlink(Symlink),
    /// A `symlinkat(2)` call.
    Symlinkat(Symlinkat),
    /// An `unlink(2)` call.
    Unlink(Unlink),
    /// An `unlinkat(2)` call.
    Unlinkat(Unlinkat),
    /// A `vfork(2)` call.
    Vfork(Vfork),
    /// A call that no dedicated type parses.
    Generic(Generic),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(syscall) => syscall.fmt(f),
            Self::Chdir(syscall) => syscall.fmt(f),
            Self::Clone(syscall) => syscall.fmt(f),
            Self::Clone3(syscall) => syscall.fmt(f),
            Self::Close(syscall) => syscall.fmt(f),
            Self::Creat(syscall) => syscall.fmt(f),
            Self::Dup(syscall) => syscall.fmt(f),
            Self::Dup2(syscall) => syscall.fmt(f),
            Self::Dup3(syscall) => syscall.fmt(f),
            Self::Execve(syscall) => syscall.fmt(f),
            Self::Execveat(syscall) => syscall.fmt(f),
            Self::Exit(exit) => exit.fmt(f),
            Self::Faccessat(syscall) => syscall.fmt(f),
            Self::Faccessat2(syscall) => syscall.fmt(f),
            Self::Fchdir(syscall) => syscall.fmt(f),
            Self::Fork(syscall) => syscall.fmt(f),
            Self::Fstat(syscall) => syscall.fmt(f),
            Self::Getdents(syscall) => syscall.fmt(f),
            Self::Getdents64(syscall) => syscall.fmt(f),
            Self::Link(syscall) => syscall.fmt(f),
            Self::Linkat(syscall) => syscall.fmt(f),
            Self::Lstat(syscall) => syscall.fmt(f),
            Self::Mkdir(syscall) => syscall.fmt(f),
            Self::Mkdirat(syscall) => syscall.fmt(f),
            Self::Newfstatat(syscall) => syscall.fmt(f),
            Self::Open(syscall) => syscall.fmt(f),
            Self::Openat(syscall) => syscall.fmt(f),
            Self::Openat2(syscall) => syscall.fmt(f),
            Self::Pipe2(syscall) => syscall.fmt(f),
            Self::Readlink(syscall) => syscall.fmt(f),
            Self::Readlinkat(syscall) => syscall.fmt(f),
            Self::Rename(syscall) => syscall.fmt(f),
            Self::Renameat(syscall) => syscall.fmt(f),
            Self::Renameat2(syscall) => syscall.fmt(f),
            Self::Rmdir(syscall) => syscall.fmt(f),
            Self::Stat(syscall) => syscall.fmt(f),
            Self::Symlink(syscall) => syscall.fmt(f),
            Self::Symlinkat(syscall) => syscall.fmt(f),
            Self::Unlink(syscall) => syscall.fmt(f),
            Self::Unlinkat(syscall) => syscall.fmt(f),
            Self::Vfork(syscall) => syscall.fmt(f),
            Self::Generic(syscall) => syscall.fmt(f),
        }
    }
}

impl FromStr for Action {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        if let Ok(syscall) = Access::from_str(s) {
            return Ok(Self::Access(syscall));
        }
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
        if let Ok(syscall) = Dup::from_str(s) {
            return Ok(Self::Dup(syscall));
        }
        if let Ok(syscall) = Dup2::from_str(s) {
            return Ok(Self::Dup2(syscall));
        }
        if let Ok(syscall) = Dup3::from_str(s) {
            return Ok(Self::Dup3(syscall));
        }
        if let Ok(syscall) = Execve::from_str(s) {
            return Ok(Self::Execve(syscall));
        }
        if let Ok(syscall) = Execveat::from_str(s) {
            return Ok(Self::Execveat(syscall));
        }
        if let Ok(exit) = Exit::from_str(s) {
            return Ok(Self::Exit(exit));
        }
        if let Ok(syscall) = Faccessat::from_str(s) {
            return Ok(Self::Faccessat(syscall));
        }
        if let Ok(syscall) = Faccessat2::from_str(s) {
            return Ok(Self::Faccessat2(syscall));
        }
        if let Ok(syscall) = Fchdir::from_str(s) {
            return Ok(Self::Fchdir(syscall));
        }
        if let Ok(syscall) = Fork::from_str(s) {
            return Ok(Self::Fork(syscall));
        }
        if let Ok(syscall) = Fstat::from_str(s) {
            return Ok(Self::Fstat(syscall));
        }
        if let Ok(syscall) = Getdents64::from_str(s) {
            return Ok(Self::Getdents64(syscall));
        }
        if let Ok(syscall) = Getdents::from_str(s) {
            return Ok(Self::Getdents(syscall));
        }
        if let Ok(syscall) = Link::from_str(s) {
            return Ok(Self::Link(syscall));
        }
        if let Ok(syscall) = Linkat::from_str(s) {
            return Ok(Self::Linkat(syscall));
        }
        if let Ok(syscall) = Lstat::from_str(s) {
            return Ok(Self::Lstat(syscall));
        }
        if let Ok(syscall) = Mkdir::from_str(s) {
            return Ok(Self::Mkdir(syscall));
        }
        if let Ok(syscall) = Mkdirat::from_str(s) {
            return Ok(Self::Mkdirat(syscall));
        }
        if let Ok(syscall) = Newfstatat::from_str(s) {
            return Ok(Self::Newfstatat(syscall));
        }
        if let Ok(syscall) = Open::from_str(s) {
            return Ok(Self::Open(syscall));
        }
        if let Ok(syscall) = Openat::from_str(s) {
            return Ok(Self::Openat(syscall));
        }
        if let Ok(syscall) = Openat2::from_str(s) {
            return Ok(Self::Openat2(syscall));
        }
        if let Ok(syscall) = Pipe2::from_str(s) {
            return Ok(Self::Pipe2(syscall));
        }
        if let Ok(syscall) = Readlinkat::from_str(s) {
            return Ok(Self::Readlinkat(syscall));
        }
        if let Ok(syscall) = Readlink::from_str(s) {
            return Ok(Self::Readlink(syscall));
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
        if let Ok(syscall) = Stat::from_str(s) {
            return Ok(Self::Stat(syscall));
        }
        if let Ok(syscall) = Symlink::from_str(s) {
            return Ok(Self::Symlink(syscall));
        }
        if let Ok(syscall) = Symlinkat::from_str(s) {
            return Ok(Self::Symlinkat(syscall));
        }
        if let Ok(syscall) = Unlink::from_str(s) {
            return Ok(Self::Unlink(syscall));
        }
        if let Ok(syscall) = Unlinkat::from_str(s) {
            return Ok(Self::Unlinkat(syscall));
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
        let is_access: fn(&Action) -> bool = |s| matches!(s, Action::Access(_));
        let is_chdir: fn(&Action) -> bool = |s| matches!(s, Action::Chdir(_));
        let is_clone: fn(&Action) -> bool = |s| matches!(s, Action::Clone(_));
        let is_clone3: fn(&Action) -> bool = |s| matches!(s, Action::Clone3(_));
        let is_close: fn(&Action) -> bool = |s| matches!(s, Action::Close(_));
        let is_creat: fn(&Action) -> bool = |s| matches!(s, Action::Creat(_));
        let is_dup: fn(&Action) -> bool = |s| matches!(s, Action::Dup(_));
        let is_dup2: fn(&Action) -> bool = |s| matches!(s, Action::Dup2(_));
        let is_dup3: fn(&Action) -> bool = |s| matches!(s, Action::Dup3(_));
        let is_execve: fn(&Action) -> bool = |s| matches!(s, Action::Execve(_));
        let is_execveat: fn(&Action) -> bool = |s| matches!(s, Action::Execveat(_));
        let is_exit: fn(&Action) -> bool = |s| matches!(s, Action::Exit(_));
        let is_faccessat: fn(&Action) -> bool = |s| matches!(s, Action::Faccessat(_));
        let is_faccessat2: fn(&Action) -> bool = |s| matches!(s, Action::Faccessat2(_));
        let is_fchdir: fn(&Action) -> bool = |s| matches!(s, Action::Fchdir(_));
        let is_fork: fn(&Action) -> bool = |s| matches!(s, Action::Fork(_));
        let is_fstat: fn(&Action) -> bool = |s| matches!(s, Action::Fstat(_));
        let is_getdents: fn(&Action) -> bool = |s| matches!(s, Action::Getdents(_));
        let is_getdents64: fn(&Action) -> bool = |s| matches!(s, Action::Getdents64(_));
        let is_link: fn(&Action) -> bool = |s| matches!(s, Action::Link(_));
        let is_linkat: fn(&Action) -> bool = |s| matches!(s, Action::Linkat(_));
        let is_lstat: fn(&Action) -> bool = |s| matches!(s, Action::Lstat(_));
        let is_mkdir: fn(&Action) -> bool = |s| matches!(s, Action::Mkdir(_));
        let is_mkdirat: fn(&Action) -> bool = |s| matches!(s, Action::Mkdirat(_));
        let is_newfstatat: fn(&Action) -> bool = |s| matches!(s, Action::Newfstatat(_));
        let is_open: fn(&Action) -> bool = |s| matches!(s, Action::Open(_));
        let is_openat: fn(&Action) -> bool = |s| matches!(s, Action::Openat(_));
        let is_openat2: fn(&Action) -> bool = |s| matches!(s, Action::Openat2(_));
        let is_pipe2: fn(&Action) -> bool = |s| matches!(s, Action::Pipe2(_));
        let is_readlink: fn(&Action) -> bool = |s| matches!(s, Action::Readlink(_));
        let is_readlinkat: fn(&Action) -> bool = |s| matches!(s, Action::Readlinkat(_));
        let is_rename: fn(&Action) -> bool = |s| matches!(s, Action::Rename(_));
        let is_renameat: fn(&Action) -> bool = |s| matches!(s, Action::Renameat(_));
        let is_renameat2: fn(&Action) -> bool = |s| matches!(s, Action::Renameat2(_));
        let is_rmdir: fn(&Action) -> bool = |s| matches!(s, Action::Rmdir(_));
        let is_stat: fn(&Action) -> bool = |s| matches!(s, Action::Stat(_));
        let is_symlink: fn(&Action) -> bool = |s| matches!(s, Action::Symlink(_));
        let is_symlinkat: fn(&Action) -> bool = |s| matches!(s, Action::Symlinkat(_));
        let is_unlink: fn(&Action) -> bool = |s| matches!(s, Action::Unlink(_));
        let is_unlinkat: fn(&Action) -> bool = |s| matches!(s, Action::Unlinkat(_));
        let is_vfork: fn(&Action) -> bool = |s| matches!(s, Action::Vfork(_));
        let is_generic: fn(&Action) -> bool = |s| matches!(s, Action::Generic(_));
        for (line, variant) in [
            ("access(\"/f\", R_OK) = 0", is_access),
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
            ("dup(0) = 3", is_dup),
            ("dup2(0, 3) = 3", is_dup2),
            ("dup3(0, 4, O_CLOEXEC) = 4", is_dup3),
            ("execve(\"/bin/echo\", [\"echo\"], []) = 0", is_execve),
            (
                "execveat(AT_FDCWD, \"prog\", [\"prog\"], [], 0) = 0",
                is_execveat,
            ),
            ("+++ exited with 0 +++", is_exit),
            ("+++ killed by SIGKILL +++", is_exit),
            ("faccessat(AT_FDCWD, \"/f\", F_OK) = 0", is_faccessat),
            (
                "faccessat2(AT_FDCWD, \"/f\", R_OK, AT_EACCESS) = 0",
                is_faccessat2,
            ),
            ("fchdir(0) = 0", is_fchdir),
            ("fork() = 123", is_fork),
            ("fstat(1, 0x1) = -1 EBADF", is_fstat),
            ("getdents(3, [], 4096) = 0", is_getdents),
            ("getdents64(3, [], 4096) = 0", is_getdents64),
            ("link(\"/a\", \"/b\") = 0", is_link),
            ("linkat(AT_FDCWD, \"a\", AT_FDCWD, \"b\", 0) = 0", is_linkat),
            ("lstat(\"/f\", 0x1) = -1 ENOENT", is_lstat),
            ("mkdir(\"/tmp/dir\", 0755) = 0", is_mkdir),
            ("mkdirat(AT_FDCWD, \"/tmp/dir\", 0755) = 0", is_mkdirat),
            (
                "newfstatat(AT_FDCWD, \"/f\", 0x1, AT_SYMLINK_NOFOLLOW) = 0",
                is_newfstatat,
            ),
            ("open(\"/tmp\", O_RDONLY) = 3", is_open),
            ("openat(4, \"file\", O_RDONLY) = 3", is_openat),
            (
                "openat2(AT_FDCWD, \"/f\", {flags=O_RDONLY, resolve=0}, 24) = 3",
                is_openat2,
            ),
            ("pipe2([3, 4], O_CLOEXEC) = 0", is_pipe2),
            ("readlink(\"/a\", \"/b\", 64) = 1", is_readlink),
            (
                "readlinkat(AT_FDCWD, \"/a\", \"/b\", 64) = 1",
                is_readlinkat,
            ),
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
            ("stat(\"/etc/hostname\", 0x7ffd684fe7b0) = 0", is_stat),
            ("symlink(\"t\", \"l\") = 0", is_symlink),
            ("symlinkat(\"t\", AT_FDCWD, \"l\") = 0", is_symlinkat),
            ("unlink(\"/f\") = 0", is_unlink),
            ("unlinkat(AT_FDCWD, \"/f\", 0) = 0", is_unlinkat),
            ("vfork() = 123", is_vfork),
            ("spill(0x1, 0x2) = 0", is_generic),
        ] {
            let syscall = Action::from_str(line).unwrap();
            assert!(variant(&syscall), "line {line:?} parsed wrongly");
        }
    }

    #[test]
    fn generic_fallback_on_invalid_known() {
        let syscall = Action::from_str("execve(\"/bin/echo\", [\"echo\", 0x1], []) = 0").unwrap();
        assert!(matches!(syscall, Action::Generic(_)));
    }

    #[test]
    fn parse_invalid() {
        assert!(Action::from_str("").is_err());
        assert!(Action::from_str("no syscall here").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "access(\"/f\", R_OK) = 0",
            "chdir(\"/tmp\") = 0",
            "close(3) = -1 EBADF",
            "dup(0) = 3",
            "dup2(1, 3) = 3",
            "dup3(0, 4, O_CLOEXEC) = 4",
            "open(\"/tmp/file\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3",
            "pipe2([3, 4], O_CLOEXEC) = 0",
            "execve(\"/bin/echo\", [\"echo\", \"hello\"], [\"PATH=/bin\"]) = 0",
            "faccessat2(AT_FDCWD, \"/f\", R_OK, AT_EACCESS) = 0",
            "unlinkat(AT_FDCWD, \"/f\", 0) = -1 ENOENT",
            "symlinkat(\"t\", AT_FDCWD, \"l\") = -1 EEXIST",
            "linkat(AT_FDCWD, \"a\", AT_FDCWD, \"b\", AT_SYMLINK_FOLLOW) = -1 EPERM",
            "clone3({flags=CLONE_VM, exit_signal=SIGCHLD}, 88) = -1 EPERM",
            "stat(\"/etc/hostname\", 0x7ffd684fe7b0) = 0",
            "spill(0x1000, -7, \"a, b\") = 0",
            "+++ exited with 3 +++",
            "+++ killed by SIGSEGV (core dumped) +++",
        ] {
            let syscall = Action::from_str(line).unwrap();
            assert_eq!(syscall.to_string(), line);
        }
    }
}
