// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! System call definitions: arguments and return values.

pub mod chdir;
pub mod clone;
pub mod clone3;
pub mod close;
pub mod creat;
pub mod dirfd;
pub mod errno;
pub mod execve;
pub mod execveat;
pub mod fchdir;
pub mod fd;
pub mod fork;
pub mod mkdir;
pub mod mkdirat;
pub mod mode;
pub mod open;
pub mod openat;
pub mod parse;
pub mod rename;
pub mod renameat;
pub mod renameat2;
pub mod result;
pub mod rmdir;
pub mod vfork;

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
pub use vfork::Vfork;
