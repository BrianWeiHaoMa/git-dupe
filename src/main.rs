//! git-dupe, the executable Git runs for `git dupe …`.
//!
//! Entry only: the words as the bytes they arrived as, the front, and the exit status.
//! The status returned here is the one place the process ends.

mod attachment;
mod bytes;
mod front;
mod guards;
mod keeper;
#[cfg(test)]
mod links_and_modes;
mod runner;

use std::process::ExitCode;

fn main() -> ExitCode {
    // Words are bytes: `std::env::args` would panic on one that is not UTF-8.
    let words: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(|word| bytes::of(&word).to_vec())
        .collect();
    ExitCode::from(front::run(&words))
}
