// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! System call definitions: arguments and return values.
//!
//! The concrete types are re-exported here; the modules that hold them are
//! private, so the layout of this directory is not part of the public API.

mod chdir;
mod clone;
mod clone3;
mod close;
mod creat;
mod dirfd;
mod errno;
mod execve;
mod execveat;
mod fchdir;
mod fd;
mod fork;
mod generic;
mod mkdir;
mod mkdirat;
mod mode;
mod open;
mod openat;
mod parse;
mod rename;
mod renameat;
mod renameat2;
mod result;
mod rmdir;
mod syscall;
mod vfork;

pub use chdir::Chdir;
pub use clone::{Clone, CloneFlags};
pub use clone3::{Clone3, Clone3Args};
pub use close::Close;
pub use creat::Creat;
pub use dirfd::DirFd;
pub use errno::Errno;
pub use execve::Execve;
pub use execveat::{Execveat, ExecveatFlags};
pub use fchdir::Fchdir;
pub use fd::FdResult;
pub use fork::{Fork, ForkResult};
pub use generic::{Arg, Generic, Ret};
pub use mkdir::Mkdir;
pub use mkdirat::Mkdirat;
pub use mode::Mode;
pub use open::{Open, OpenFlags};
pub use openat::Openat;
pub use parse::ParseError;
pub use rename::Rename;
pub use renameat::Renameat;
pub use renameat2::{Renameat2, Renameat2Flags};
pub use result::OkResult;
pub use rmdir::Rmdir;
pub use syscall::Syscall;
pub use vfork::Vfork;
