//! Taking the public places of a command, and the refusal of a word that names one
//! (G18, `Holds/G18`). `Guards` builds the places and answers the question; this takes
//! what they are built from and writes the line.
//!
//! What the code below cannot show:
//!
//! - The places are taken once per command, by two public runs that only read (G5, R2)
//!   and whatever the repositories hold, a fixed two (G22, R4): `git config -z
//!   --get-regexp` for every URL of every remote, without `--local`, so that a remote the
//!   global configuration or a `-c` before `dupe` defines is a remote of the project too;
//!   and `git worktree list --porcelain -z` for every working tree root. The Git
//!   directories are the common one and each entry directly under its `worktrees/`.
//!   Nothing of the private repository is needed, so that `clone` can take them where
//!   none exists.
//! - Each run is git-dupe's own: `config` its first word, so that `GIT_CONFIG` is not
//!   read (`runner::environment`), and an alias chain's prefix before it, as before every
//!   run once a chain is dispatched. `config` exiting 1 is no remote. Any other failing
//!   end of a run ends the command as Git ended it, its message already on standard
//!   error, and an answer that cannot be read, or a `worktrees/` that cannot be listed,
//!   is a refusal: the command never runs without its places.
//! - The refusal of a word is written here and nowhere else; `clone` refuses with it too.
//!   It names the word as typed (`lines::typed`, on one line whatever it holds), the
//!   place, and `git dupe git` for a destination the developer means. A place is named
//!   by `described` in every line that names one: a remote by its name and not by its
//!   URL, which may hold a credential (G25).

use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use super::lines;
use super::outcome::{self, Outcome};
use crate::guards::destination::ReadFrom;
use crate::guards::places::{Place, Places};
use crate::runner::locate::Facts;
use crate::runner::records;
use crate::runner::{End, Run};

/// The places of the project that `facts` locate.
pub fn take(facts: &Facts) -> Result<Places, Outcome> {
    let urls = asked(
        Run::public([
            "config",
            "-z",
            "--get-regexp",
            r"^remote\..*\.(url|pushurl)$",
        ]),
        true,
    )?;
    let mut remotes = Vec::new();
    for (key, url) in records::config_records(&urls) {
        let name = key
            .strip_prefix(b"remote.")
            .and_then(|rest| Some(&rest[..rest.iter().rposition(|&byte| byte == b'.')?]));
        let Some(name) = name else {
            return Err(outcome::unreadable());
        };
        // A key without a value gives the remote no URL: Git refuses to read it as one.
        if let Some(url) = url {
            remotes.push((name, url));
        }
    }
    let listing = asked(
        Run::public(["worktree", "list", "--porcelain", "-z"]),
        false,
    )?;
    let roots = records::worktree_roots(&listing).ok_or_else(outcome::unreadable)?;
    let worktrees = facts.common_directory().join("worktrees");
    let mut git_directories = vec![facts.common_directory().as_os_str().as_bytes().to_vec()];
    match fs::read_dir(&worktrees) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|cause| unlisted(&worktrees, &cause))?;
                git_directories.push(entry.path().into_os_string().into_vec());
            }
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(unlisted(&worktrees, &cause)),
    }
    let from = ReadFrom {
        root: facts.root(),
        outside: facts.outside(),
        home: facts.home().map(OsStrExt::as_bytes),
    };
    Ok(Places::new(
        remotes,
        roots,
        git_directories.iter().map(Vec::as_slice),
        from,
    ))
}

/// The answer of one run a guard needs, captured: exit 0, or exit 1 where that is an
/// empty answer.
pub fn asked(run: Run, one_is_none: bool) -> Result<Vec<u8>, Outcome> {
    let finished = run.capture_output().start().map_err(outcome::not_started)?;
    match finished.end {
        End::Code(0) => Ok(finished.stdout),
        End::Code(1) if one_is_none => Ok(Vec::new()),
        end => Err(Outcome::Git(end)),
    }
}

/// The refusal of a `worktrees/` that cannot be listed, whose entries are places.
fn unlisted(directory: &Path, cause: &io::Error) -> Outcome {
    let mut line = b"cannot list the project's linked worktrees in ".to_vec();
    line.extend_from_slice(directory.as_os_str().as_bytes());
    line.extend_from_slice(format!(": {cause}").as_bytes());
    outcome::refuse(&line)
}

/// The refusal of `word`, which names `place`.
pub fn refuse(word: &[u8], place: &Place) -> Outcome {
    let mut line = lines::typed(word);
    line.extend_from_slice(b" names ");
    line.extend_from_slice(&described(place));
    line.extend_from_slice(b"; 'git dupe git' runs Git unguarded, for a destination you mean");
    outcome::refuse(&line)
}

/// `place` as a line names it: what it is, then which, between single quotes.
pub fn described(place: &Place) -> Vec<u8> {
    let (what, bytes): (&[u8], &[u8]) = match place {
        Place::Remote { name, .. } => (b"a URL of the project's remote ", name),
        Place::WorkingTree(path) => (b"the project's working tree ", path),
        Place::GitEntry(path) => (b"the .git entry of the project's working tree ", path),
        Place::GitDirectory(path) => (b"the project's Git directory ", path),
    };
    [what, b"'", bytes, b"'"].concat()
}
