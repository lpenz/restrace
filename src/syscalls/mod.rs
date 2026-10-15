// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! System call definitions: arguments and return values.
//!
//! The concrete types are re-exported here; the modules that hold them are
//! private, so the layout of this directory is not part of the public API.
//!
//! The [`Action`](crate::Action) enum that unifies these records with the
//! process [`Exit`](crate::Exit) lives at the crate root.

mod access;
mod accessmode;
mod buffer;
mod chdir;
mod clone;
mod clone3;
mod close;
mod creat;
mod device;
mod dirfd;
mod dup;
mod dup2;
mod dup3;
mod errno;
mod execve;
mod execveat;
mod faccessat;
mod faccessat2;
mod fchdir;
mod fd;
mod filemode;
mod flags;
mod fork;
mod generic;
mod link;
mod linkat;
mod mkdir;
mod mkdirat;
mod mode;
mod open;
mod openat;
mod parse;
mod pipe2;
mod rename;
mod renameat;
mod renameat2;
mod result;
mod rmdir;
mod stat;
mod statbuf;
mod symlink;
mod symlinkat;
mod unlink;
mod unlinkat;
mod vfork;

pub use access::Access;
pub use accessmode::AccessMode;
pub use buffer::Buffer;
pub use chdir::Chdir;
pub use clone::{Clone, CloneFlags};
pub use clone3::{Clone3, Clone3Args};
pub use close::Close;
pub use creat::Creat;
pub use device::Device;
pub use dirfd::DirFd;
pub use dup::Dup;
pub use dup2::Dup2;
pub use dup3::{Dup3, Dup3Flags};
pub use errno::Errno;
pub use execve::Execve;
pub use execveat::{Execveat, ExecveatFlags};
pub use faccessat::Faccessat;
pub use faccessat2::{Faccessat2, FaccessatFlags};
pub use fchdir::Fchdir;
pub use fd::FdResult;
pub use filemode::FileMode;
pub use fork::{Fork, ForkResult};
pub use generic::{Arg, Generic, Ret};
pub use link::Link;
pub use linkat::{Linkat, LinkatFlags};
pub use mkdir::Mkdir;
pub use mkdirat::Mkdirat;
pub use mode::Mode;
pub use open::{Open, OpenFlags};
pub use openat::Openat;
pub use parse::ParseError;
pub use pipe2::{Pipe2, Pipe2Flags};
pub use rename::Rename;
pub use renameat::Renameat;
pub use renameat2::{Renameat2, Renameat2Flags};
pub use result::OkResult;
pub use rmdir::Rmdir;
pub use stat::Stat;
pub use statbuf::StatBuf;
pub use symlink::Symlink;
pub use symlinkat::Symlinkat;
pub use unlink::Unlink;
pub use unlinkat::{Unlinkat, UnlinkatFlags};
pub use vfork::Vfork;
