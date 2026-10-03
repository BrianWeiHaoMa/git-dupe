//! git-dupe, the executable Git runs for `git dupe …`.
//!
//! Entry only: the words as the bytes they arrived as, the front, and the exit status.
//! The status returned here is the one place the process ends.

mod attachment;
mod front;
mod guards;
mod keeper;
mod runner;

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Words are bytes: `std::env::args` would panic on one that is not UTF-8.
    let words: Vec<OsString> = std::env::args_os().skip(1).collect();
    ExitCode::from(front::run(&words))
}
