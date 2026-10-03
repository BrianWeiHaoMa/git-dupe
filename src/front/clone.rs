//! `clone`'s words, its handler, and the lines it prints around Git's own.
//!
//! The words are read before anything else (`Composition/Front` step 2): a help request
//! anywhere before `--` is answered without locating, and a misuse is reported after the
//! locate run, so that outside a repository Git's own message ends the command. `-b`
//! takes the next word as its value, whatever it is, and a later one replaces an earlier
//! one; `--` ends the options. A `URL` or `BRANCH` beginning with `-` is a misuse: each
//! reaches a Git step as a plain word, where Git would read it as an option of its own,
//! and a path that begins with `-` can be written `./-x`.
//!
//! The handler takes the public places, which need no private repository, and leaves the
//! rest to the attachment part, which refuses or attaches. Settle follows in the
//! sequence, whatever this returns: after the refusal in an attached workspace, after a
//! step that failed once the workspace is attached, and after success.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::places;
use crate::attachment::{self, Cloned, Kept, NotCloned, Standing, Written};
use crate::runner::locate::Workspace;

/// What `clone`'s words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked<'w> {
    /// `-h` or `--help` before `--`.
    Help,
    /// Attach from `url`, checking out `branch` when one was given.
    Clone {
        url: &'w OsStr,
        branch: Option<&'w OsStr>,
    },
    /// Not `clone URL [-b BRANCH]`: the fault, which names nothing typed.
    Misused(&'static [u8]),
}

const NO_URL: &[u8] = b"'git dupe clone' needs the URL of a private repository";
const AN_OPTION: &[u8] = b"'git dupe clone' takes no option but -b";
const NO_BRANCH: &[u8] = b"-b needs a branch name";
const AN_OPERAND: &[u8] = b"'git dupe clone' takes one URL and no directory: it attaches the \
    repository it is run in";
const DASHED_URL: &[u8] = b"the URL cannot begin with '-'; write a path that does as ./<path>";
const DASHED_BRANCH: &[u8] = b"the branch name cannot begin with '-'";

/// Reads `clone URL [-b BRANCH]`, the options before or after `URL`, and `URL` after
/// `--` too.
pub fn read(words: &[OsString]) -> Asked<'_> {
    let end_of_options = words
        .iter()
        .position(|word| word.as_bytes() == b"--")
        .unwrap_or(words.len());
    if words[..end_of_options]
        .iter()
        .any(|word| matches!(word.as_bytes(), b"-h" | b"--help"))
    {
        return Asked::Help;
    }
    let (mut url, mut branch) = (None, None);
    let mut options = true;
    let mut words = words.iter();
    while let Some(word) = words.next() {
        let bytes = word.as_bytes();
        if options && bytes == b"-b" {
            match words.next() {
                Some(value) => branch = Some(value.as_os_str()),
                None => return Asked::Misused(NO_BRANCH),
            }
        } else if options && bytes == b"--" {
            options = false;
        } else if options && bytes.starts_with(b"-") {
            return Asked::Misused(AN_OPTION);
        } else if url.is_none() {
            url = Some(word.as_os_str());
        } else {
            return Asked::Misused(AN_OPERAND);
        }
    }
    let Some(url) = url else {
        return Asked::Misused(NO_URL);
    };
    if url.as_bytes().starts_with(b"-") {
        return Asked::Misused(DASHED_URL);
    }
    if branch.is_some_and(|branch| branch.as_bytes().starts_with(b"-")) {
        return Asked::Misused(DASHED_BRANCH);
    }
    Asked::Clone { url, branch }
}

/// The handler: `words` are every word after `clone`, each compared with the public
/// places.
pub fn run(
    workspace: &Workspace,
    words: &[OsString],
    url: &OsStr,
    branch: Option<&OsStr>,
) -> Outcome {
    let places = match places::take(&workspace.facts()) {
        Ok(places) => places,
        Err(ended) => return ended,
    };
    match attachment::clone(workspace, &places, words, url, branch) {
        Ok(Cloned::CheckedOut(written)) => {
            warn(&written);
            Outcome::Answered
        }
        Ok(Cloned::NoBranch) => {
            lines::write(
                Level::Hint,
                b"the private remote has no branch yet, so nothing was checked out; \
                  commit here, then 'git dupe push -u origin HEAD' starts its history",
            );
            Outcome::Answered
        }
        Err(NotCloned::Attached) => outcome::refuse(
            b"this workspace is already attached to a private repository; to start over \
              from a remote, run 'git dupe detach', which refuses while private work is \
              not on a remote ('git dupe detach --force' removes it with the repository), \
              then 'git dupe clone' again",
        ),
        Err(NotCloned::Refused(refusal)) => outcome::refuse_to_attach(workspace, refusal),
        Err(NotCloned::NamesAPublicPlace(word, place)) => places::refuse(&word, &place),
        Err(NotCloned::NoDefaultBranch) => outcome::refuse(
            b"the private remote's HEAD names no branch it has, so there is no default \
              branch to check out; run 'git dupe detach', or 'git dupe detach --force' \
              where it refuses, then 'git dupe clone' again with -b BRANCH",
        ),
        Err(NotCloned::Failed(failed)) => outcome::failed(failed),
    }
}

/// One `warning:` per path the write step left as it was: each obstruction, then each
/// kept file, in byte order. A path is relative to the root, as Git listed it, and so is
/// the command each line names, wherever `clone` was run.
fn warn(written: &Written) {
    for (path, standing) in &written.obstructions {
        lines::write(Level::Warning, &obstructed(path, *standing));
    }
    for (path, why) in &written.kept {
        lines::write(Level::Warning, &kept(path, why));
    }
}

/// The line of a path that is not a directory where private files lie below it.
fn obstructed(path: &[u8], standing: Standing) -> Vec<u8> {
    let what: &[u8] = match standing {
        Standing::SymbolicLink => {
            b" was kept: it is a symbolic link, which Git does not \
            write through, so the private files below it were not written; move it aside, \
            then 'git dupe restore -- "
        }
        Standing::File => {
            b" was kept: it is a file, not a directory, so the private files \
            below it were not written; move it aside, then 'git dupe restore -- "
        }
    };
    [path, what, path, b"' run from the root writes them"].concat()
}

/// The line of a present path that differs from the checked-out version.
fn kept(path: &[u8], why: &Kept) -> Vec<u8> {
    match why {
        Kept::Differs => [
            path,
            b" was kept as it was and differs from the private repository's version; \
              'git dupe commit -a' keeps it, 'git dupe restore -- ",
            path,
            b"' run from the root replaces it",
        ]
        .concat(),
        Kept::Directory => [
            path,
            b" was kept: it is a directory where the private repository has a file; \
              move it aside, then 'git dupe restore -- ",
            path,
            b"' run from the root writes the file",
        ]
        .concat(),
        Kept::BeyondALink(link) => [
            path,
            b" was kept: it lies beyond the symbolic link ",
            link,
            b", where Git does not look; move the link aside, then 'git dupe restore -- ",
            link,
            b"' run from the root writes the private files below it",
        ]
        .concat(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    fn cloned<'w>(url: &'w str, branch: Option<&'w str>) -> Asked<'w> {
        Asked::Clone {
            url: OsStr::new(url),
            branch: branch.map(OsStr::new),
        }
    }

    #[test]
    fn clone_reads_one_url_and_one_branch() {
        assert_eq!(read(&words(&["u"])), cloned("u", None));
        assert_eq!(read(&words(&["u", "-b", "x"])), cloned("u", Some("x")));
        assert_eq!(read(&words(&["-b", "x", "u"])), cloned("u", Some("x")));
        assert_eq!(read(&words(&["--", "u"])), cloned("u", None));
        assert_eq!(read(&words(&["u", "--"])), cloned("u", None));
        assert_eq!(
            read(&words(&["-b", "x", "--", "u"])),
            cloned("u", Some("x"))
        );
        assert_eq!(read(&words(&["./-x"])), cloned("./-x", None));
        assert_eq!(read(&words(&["x=y"])), cloned("x=y", None));
        assert_eq!(read(&words(&[""])), cloned("", None));
        // The last branch given stands.
        assert_eq!(
            read(&words(&["-b", "x", "u", "-b", "feature/y"])),
            cloned("u", Some("feature/y"))
        );
        let not_utf8 = [
            OsStr::from_bytes(b"caf\xe9").to_owned(),
            OsString::from("-b"),
            OsStr::from_bytes(b"\xff").to_owned(),
        ];
        assert_eq!(
            read(&not_utf8),
            Asked::Clone {
                url: OsStr::from_bytes(b"caf\xe9"),
                branch: Some(OsStr::from_bytes(b"\xff")),
            }
        );
    }

    #[test]
    fn any_other_word_is_a_misuse() {
        for (misuse, fault) in [
            (&[][..], NO_URL),
            (&["--"], NO_URL),
            (&["-b", "x"], NO_URL),
            (&["u", "extra"], AN_OPERAND),
            (&["u", "--", "extra"], AN_OPERAND),
            (&["--", "u", "extra"], AN_OPERAND),
            (&["--depth", "1", "u"], AN_OPTION),
            (&["u", "--depth=1"], AN_OPTION),
            (&["--branch", "x", "u"], AN_OPTION),
            (&["--branch=x", "u"], AN_OPTION),
            (&["-bx", "u"], AN_OPTION),
            (&["-q", "u"], AN_OPTION),
            (&["-", "u"], AN_OPTION),
            (&["u", "-b"], NO_BRANCH),
            (&["-b"], NO_BRANCH),
            // A word Git would read as an option of its own.
            (&["--", "-x"], DASHED_URL),
            (&["--", "--upload-pack=x"], DASHED_URL),
            (&["u", "-b", "--list"], DASHED_BRANCH),
            (&["-b", "--", "u"], DASHED_BRANCH),
            (&["-b", "-", "u"], DASHED_BRANCH),
        ] {
            assert_eq!(read(&words(misuse)), Asked::Misused(fault), "{misuse:?}");
        }
    }

    #[test]
    fn a_help_request_anywhere_before_the_end_of_options_decides() {
        for request in [
            &["-h"][..],
            &["--help"],
            &["u", "-h"],
            &["-b", "-h"],
            &["--depth", "1", "--help"],
            &["u", "extra", "-h", "--", "y"],
        ] {
            assert_eq!(read(&words(request)), Asked::Help, "{request:?}");
        }
        assert_eq!(read(&words(&["--", "-h"])), Asked::Misused(DASHED_URL));
    }

    #[test]
    fn each_warning_names_its_path_and_the_command_that_writes_the_private_version() {
        let lines = [
            obstructed(b"notes", Standing::File),
            obstructed(b"lnk", Standing::SymbolicLink),
            kept(b".env.local", &Kept::Differs),
            kept(b"conf", &Kept::Directory),
            kept(b"lnk/a", &Kept::BeyondALink(b"lnk".to_vec())),
        ];
        let named = [&b"notes"[..], b"lnk", b".env.local", b"conf", b"lnk"];
        for (line, path) in lines.iter().zip(named) {
            let restore = [&b"'git dupe restore -- "[..], path, b"'"].concat();
            assert!(
                line.windows(restore.len()).any(|w| w == restore),
                "{line:?}"
            );
        }
        for (index, line) in lines.iter().enumerate() {
            assert!(lines[..index].iter().all(|earlier| earlier != line));
        }
    }
}
