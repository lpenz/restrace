// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! System call definitions: arguments and return values.

pub mod dirfd;
pub mod errno;
pub mod open;
pub mod openat;
pub mod parse;

pub use dirfd::DirFd;
pub use errno::Errno;
pub use open::{Mode, Open, OpenFlags, OpenResult};
pub use openat::Openat;
pub use parse::ParseError;