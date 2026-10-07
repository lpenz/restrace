// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! System call definitions: arguments and return values.

pub mod creat;
pub mod dirfd;
pub mod errno;
pub mod fd;
pub mod mode;
pub mod open;
pub mod openat;
pub mod parse;

pub use creat::Creat;
pub use dirfd::DirFd;
pub use errno::Errno;
pub use fd::FdResult;
pub use mode::Mode;
pub use open::{Open, OpenFlags};
pub use openat::Openat;
pub use parse::ParseError;