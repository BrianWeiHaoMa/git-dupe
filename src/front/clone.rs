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

use super::git_version;
use super::help;
use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::places;
use crate::attachment::{self, Cloned, Kept, NotCloned, Standing, Written};
use crate::guards::pathspec;
use crate::guards::quoted::shell_word;
use crate::runner::locate::Workspace;

/// What `clone`'s words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked<'w> {
    /// `-h` or `--help` before `--`.
    Help,
    /// Attach from `url`, checking out `branch` when one was given.
    Clone {
        url: &'w [u8],
        branch: Option<&'w [u8]>,
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
pub fn read(words: &[Vec<u8>]) -> Asked<'_> {
    if help::requested(words) {
        return Asked::Help;
    }
    let (mut url, mut branch) = (None, None);
    let mut options = true;
    let mut words = words.iter();
    while let Some(word) = words.next() {
        let bytes = word.as_slice();
        if options && bytes == b"-b" {
            match words.next() {
                Some(value) => branch = Some(value.as_slice()),
                None => return Asked::Misused(NO_BRANCH),
            }
        } else if options && bytes == b"--" {
            options = false;
        } else if options && bytes.starts_with(b"-") {
            return Asked::Misused(AN_OPTION);
        } else if url.is_none() {
            url = Some(word.as_slice());
        } else {
            return Asked::Misused(AN_OPERAND);
        }
    }
    let Some(url) = url else {
        return Asked::Misused(NO_URL);
    };
    if url.starts_with(b"-") {
        return Asked::Misused(DASHED_URL);
    }
    if branch.is_some_and(|branch| branch.starts_with(b"-")) {
        return Asked::Misused(DASHED_BRANCH);
    }
    Asked::Clone { url, branch }
}

/// The handler: names a Git older than F4's range first, before the runs of the public
/// places such a Git can fail; `words` are every word after `clone`, each compared with
/// the public places.
pub fn run(workspace: &Workspace, words: &[Vec<u8>], url: &[u8], branch: Option<&[u8]>) -> Outcome {
    git_version::warn_when_too_old();
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
        Err(NotCloned::Unresolved(unresolved)) => outcome::unresolved(&unresolved),
        // The steps after the line did not run, which only a new `clone` takes (G2).
        Err(NotCloned::Unwritten(unwritten)) => outcome::unwritten(
            &unwritten,
            b"'git dupe detach --force', then 'git dupe clone' again, start over",
        ),
        Err(NotCloned::NoDefaultBranch) => outcome::refuse(
            b"the private remote's HEAD names no branch it has, so there is no default \
              branch to check out; run 'git dupe detach', or 'git dupe detach --force' \
              where it refuses, then 'git dupe clone' again with -b BRANCH",
        ),
        Err(NotCloned::FetchFailed(failed)) => {
            lines::write(Level::Hint, FETCH_FAILED);
            outcome::failed(failed)
        }
        Err(NotCloned::Failed(failed)) => outcome::failed(failed),
    }
}

/// The hint after the fetch failed, which follows Git's own lines and comes before
/// settle's warnings: the workspace this invocation attached has nothing checked out, and
/// the commands that start over. A fetch that failed part way can have brought a commit a
/// tag names, which `detach` without `--force` refuses to lose, so the line names both as
/// the attached workspace's refusal does.
const FETCH_FAILED: &[u8] = b"the fetch failed, so this worktree is now attached to a \
    private repository with nothing checked out, and no private file was written; to \
    start over, run 'git dupe detach', or 'git dupe detach --force' where it refuses, \
    then 'git dupe clone <url>' once <url> can be fetched";

/// One `warning:` per path the write step left as it was: each obstruction, then each
/// kept file, then each file left unwritten, in byte order. A path is relative to the root,
/// as Git listed it, and so is the command each line names, wherever `clone` was run. No
/// line offers a `restore` that would name a path at or below which the project tracks a
/// path: written there, the private version would change the project's file, or replace
/// what holds it, and a public `git add` would stage that (N9); such a line names
/// `show_command` instead, which prints the private version.
fn warn(written: &Written) {
    for (path, standing) in &written.obstructions {
        let line = obstructed(path, *standing, written.project_tracks(path));
        lines::write(Level::Warning, &line);
    }
    for (path, why) in &written.kept {
        let restored = match why {
            Kept::BeyondALink(link) => link,
            Kept::Differs | Kept::Directory => path,
        };
        let line = kept(path, why, written.project_tracks(restored));
        lines::write(Level::Warning, &line);
    }
    for path in &written.unwritten {
        lines::write(Level::Warning, &unwritten(path));
    }
}

/// The line of a path that is not a directory where private files lie below it.
fn obstructed(path: &[u8], standing: Standing, project_tracks: bool) -> Vec<u8> {
    let what: &[u8] = match standing {
        Standing::SymbolicLink => {
            b" was kept: it is a symbolic link, so the private files below it were not \
            written"
        }
        Standing::File => {
            b" was kept: it is a file, not a directory, so the private files below it were \
            not written"
        }
    };
    let listing = listing_command(path);
    let then: &[&[u8]] = if project_tracks {
        &[
            b"; the project tracks a path at or below it, so they are not to be written \
              there: '",
            &listing,
            b"' run from the root lists them, and 'git dupe show HEAD:<file>' prints each",
        ]
    } else {
        &[
            b"; move it aside, then '",
            &restore_command(path),
            b"' run from the root writes them",
        ]
    };
    [&[path, what][..], then].concat().concat()
}

/// The line of a present path that differs from the checked-out version, where the project
/// tracks, or does not, a path at or below the one a restore would name.
fn kept(path: &[u8], why: &Kept, project_tracks: bool) -> Vec<u8> {
    match why {
        Kept::Differs if project_tracks => [
            path,
            b" was kept: the project tracks it, so this is the project's file, and the \
              private version written there would be a change to it that a public \
              'git add' stages; '",
            &show_command(path),
            b"' prints the private version, to keep under another name",
        ]
        .concat(),
        Kept::Differs => [
            path,
            b" was kept as it was and differs from the private repository's version; '",
            &commit_command(path),
            b"' run from the root keeps it, '",
            &restore_command(path),
            b"' run from the root replaces it",
        ]
        .concat(),
        Kept::Directory if project_tracks => [
            path,
            b" was kept: it is a directory where the private repository has a file, and \
              the project tracks a path at or below it, so the file is not to be written \
              there; '",
            &show_command(path),
            b"' prints it",
        ]
        .concat(),
        Kept::Directory => [
            path,
            b" was kept: it is a directory where the private repository has a file; \
              move it aside, then '",
            &restore_command(path),
            b"' run from the root writes the file",
        ]
        .concat(),
        Kept::BeyondALink(link) if project_tracks => [
            path,
            b" was kept: it lies beyond the symbolic link ",
            link,
            b", and the project tracks a path at or below that link, so the private files \
              are not to be written there; '",
            &show_command(path),
            b"' prints this one",
        ]
        .concat(),
        Kept::BeyondALink(link) => [
            path,
            b" was kept: it lies beyond the symbolic link ",
            link,
            b"; move the link aside, then '",
            &restore_command(link),
            b"' run from the root writes the private files below it",
        ]
        .concat(),
    }
}

/// The line of an absent file the project tracks, which the write step left unwritten.
fn unwritten(path: &[u8]) -> Vec<u8> {
    [
        path,
        b" was not written: the project tracks it and its file is missing from disk, and \
          the private version written there would be a change to the project's file that \
          a public 'git add' stages; '",
        &show_command(path),
        b"' prints the private version",
    ]
    .concat()
}

/// `git dupe commit` of `path` alone, which keeps the file as it stands.
fn commit_command(path: &[u8]) -> Vec<u8> {
    pathspec::offered_command(b"dupe commit", path)
}

/// `git dupe show HEAD:<path>`, which prints the private version from any directory: Git
/// reads the path after `HEAD:` from the root of the commit's tree, and as no pathspec, so
/// no magic or setting applies; the revision and path are one word as a shell reads it.
fn show_command(path: &[u8]) -> Vec<u8> {
    [
        &b"git dupe show "[..],
        &shell_word(&[&b"HEAD:"[..], path].concat()),
    ]
    .concat()
}

/// `git dupe ls-files` of `path` alone, which lists the private files at or below it.
fn listing_command(path: &[u8]) -> Vec<u8> {
    pathspec::offered_command(b"dupe ls-files", path)
}

/// `git dupe restore` of `path` alone, which Git runs, as a line offers it (G25).
fn restore_command(path: &[u8]) -> Vec<u8> {
    pathspec::offered_command(b"dupe restore", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> Vec<Vec<u8>> {
        words.iter().map(|word| word.as_bytes().to_vec()).collect()
    }

    fn cloned<'w>(url: &'w str, branch: Option<&'w str>) -> Asked<'w> {
        Asked::Clone {
            url: url.as_bytes(),
            branch: branch.map(str::as_bytes),
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
        let not_utf8 = [b"caf\xe9".to_vec(), b"-b".to_vec(), b"\xff".to_vec()];
        assert_eq!(
            read(&not_utf8),
            Asked::Clone {
                url: &b"caf\xe9"[..],
                branch: Some(&b"\xff"[..]),
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
            obstructed(b"notes", Standing::File, false),
            obstructed(b"lnk", Standing::SymbolicLink, false),
            kept(b".env.local", &Kept::Differs, false),
            kept(b"conf", &Kept::Directory, false),
            kept(b"lnk/a", &Kept::BeyondALink(b"lnk".to_vec()), false),
        ];
        let named = [&b"notes"[..], b"lnk", b".env.local", b"conf", b"lnk"];
        for (line, path) in lines.iter().zip(named) {
            let restore = [&b"'git dupe restore -- "[..], path, b"'"].concat();
            assert!(holds(line, &restore), "{line:?}");
        }
        for (index, line) in lines.iter().enumerate() {
            assert!(lines[..index].iter().all(|earlier| earlier != line));
        }
        // The file kept as it was is kept by committing that one path.
        assert!(holds(
            &lines[2],
            b"'git dupe commit -- .env.local' run from the root"
        ));
    }

    #[test]
    fn no_warning_offers_a_restore_where_the_project_tracks_a_path_at_or_below_it() {
        let lines = [
            obstructed(b"notes", Standing::File, true),
            obstructed(b"lnk", Standing::SymbolicLink, true),
            kept(b"AGENTS.md", &Kept::Differs, true),
            kept(b"conf", &Kept::Directory, true),
            kept(b"lnk/a", &Kept::BeyondALink(b"lnk".to_vec()), true),
            unwritten(b"README.md"),
        ];
        for line in &lines {
            assert!(!holds(line, b"restore"), "{line:?}");
            assert!(!holds(line, b"commit"), "{line:?}");
            assert!(holds(line, b"'git dupe show HEAD:"), "{line:?}");
            assert!(holds(line, b"the project tracks"), "{line:?}");
        }
        assert!(holds(&lines[2], b"'git dupe show HEAD:AGENTS.md'"));
        assert!(holds(&lines[5], b"'git dupe show HEAD:README.md'"));
    }

    #[test]
    fn the_shown_version_is_one_shell_word_and_no_pathspec() {
        assert_eq!(show_command(b"AGENTS.md"), b"git dupe show HEAD:AGENTS.md");
        assert_eq!(
            show_command(b"two $words/a b"),
            b"git dupe show \"HEAD:two \\$words/a b\""
        );
        assert_eq!(
            show_command(b"star*.txt"),
            b"git dupe show \"HEAD:star*.txt\""
        );
        // An obstruction above a path the project tracks names how to list what it holds.
        assert!(holds(
            &obstructed(b"two $words", Standing::File, true),
            b"'git dupe ls-files -- \"two \\$words\"' run from the root lists them"
        ));
    }

    fn holds(line: &[u8], part: &[u8]) -> bool {
        line.windows(part.len()).any(|window| window == part)
    }
}
