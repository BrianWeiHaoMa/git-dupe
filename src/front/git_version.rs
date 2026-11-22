//! The one question `init` and `clone` ask of Git's version, and the `warning:` they give
//! for a Git older than F4's range.
//!
//! What the code below cannot show:
//!
//! - It is asked by `init` and `clone` alone, as the first thing their handlers do once the
//!   workspace is located, so that the line stands before every run an older Git can fail:
//!   `clone`'s comparison of public places, whose `worktree list -z` Git 2.34.1 refuses,
//!   among them. No other command asks it, and `clone` asks it once, not again for the
//!   steps it shares with `init` (G22).
//! - The question is one public `git version`, which needs no repository and writes
//!   nothing (R2); the answer is its line, read by `records::git_version` (R5), never a
//!   message of Git's.
//! - Only a release below the floor gets the line, the command going on as it would
//!   without it: a supported Git gets none, and so do a release at or past 3.0.0, the upper
//!   bound being F4's alone, and an answer that names no release or a run that fails,
//!   which say nothing of the Git found (E5).

use super::lines::{self, Level};
use crate::runner::records::{self, GitVersion};
use crate::runner::{End, Run};

/// The oldest release F4 supports.
const FLOOR: (u32, u32, u32) = (2, 43, 0);

/// F4's range, as the line and `git dupe help` name it.
const SUPPORTED: &[u8] = b"Git 2.43.0 or newer, below 3.0.0";

/// Asks Git its version and writes the `warning:` when it is older than the floor.
pub fn warn_when_too_old() {
    let Ok(finished) = Run::public(["version"]).capture_output().start() else {
        return;
    };
    if finished.end != End::Code(0) {
        return;
    }
    if let Some(line) = records::git_version(&finished.stdout).and_then(|found| too_old(&found)) {
        lines::write(Level::Warning, &line);
    }
}

/// The line for the version `found`, where it is older than the floor.
fn too_old(found: &GitVersion) -> Option<Vec<u8>> {
    (found.release < FLOOR).then(|| {
        [
            &b"the git on PATH is version "[..],
            found.spelled,
            b", and git-dupe supports ",
            SUPPORTED,
            b": with this one, this command or a later one can fail with Git's own message",
        ]
        .concat()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(answer: &[u8]) -> Option<Vec<u8>> {
        too_old(&records::git_version(answer).expect("a version answer"))
    }

    #[test]
    fn a_release_below_the_floor_is_named_with_the_range() {
        for (answer, spelled) in [
            (&b"git version 2.34.1\n"[..], &b"2.34.1"[..]),
            (b"git version 2.42.4\n", b"2.42.4"),
            (
                b"git version 2.42.4 (Apple Git-150)\n",
                b"2.42.4 (Apple Git-150)",
            ),
            (b"git version 1.8.5.6\n", b"1.8.5.6"),
        ] {
            let line = line(answer).expect("a line");
            let named = [&b"the git on PATH is version "[..], spelled, b", "].concat();
            assert!(line.starts_with(&named), "{line:?}");
            assert!(
                line.windows(SUPPORTED.len()).any(|part| part == SUPPORTED),
                "{line:?}"
            );
        }
    }

    #[test]
    fn a_supported_release_or_one_past_the_range_gets_no_line() {
        for answer in [
            &b"git version 2.43.0\n"[..],
            b"git version 2.43.0.rc1\n",
            b"git version 2.50.1 (Apple Git-155)\n",
            b"git version 2.55.0.windows.5\n",
            b"git version 2.56.0\n",
            b"git version 3.0.0\n",
        ] {
            assert_eq!(line(answer), None, "{answer:?}");
        }
    }

    #[test]
    fn the_range_is_the_one_the_general_help_names() {
        let floor = format!("{}.{}.{}", FLOOR.0, FLOOR.1, FLOOR.2);
        assert!(SUPPORTED.starts_with(format!("Git {floor} ").as_bytes()));
        let general = include_str!("help/general.txt").replace('\n', " ");
        assert!(
            general.contains(std::str::from_utf8(SUPPORTED).unwrap()),
            "{general}"
        );
    }
}
