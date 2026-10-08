//! `clone` after its words (G2, `Holds/G2`): the refusals, then Git's steps, each a
//! private run made from the root, then the write step.
//!
//! The refusals come before any step, in this order: a workspace already attached; G4's,
//! as `init` decides them; a word after `clone`, whole or after its first `=`, that names
//! a public place (G18), every word asked, `-b`'s value included. Step 1 is where the
//! workspace becomes attached, so that a refusal leaves nothing behind.
//!
//! The steps: `init` as G1 does it without `-b`; `remote add origin URL`; `fetch origin`,
//! its output Git's; `for-each-ref` of `origin`'s branches, whose empty answer ends the
//! command with nothing checked out; without `-b`, `ls-remote --symref origin HEAD`, whose
//! `ref:` line for `HEAD` names the branch, and whose lack of one refuses; `branch --track`
//! from `refs/remotes/origin/BRANCH`, `symbolic-ref HEAD`, and `reset -q`, which sets the
//! index and touches no file (S7); then
//! the write step (`write`). No step passes `--force` or runs `checkout`, `switch`, or
//! `restore`. The standard output of every step other than `init` and `fetch` is captured
//! and dropped, `branch --track`'s line among it.
//!
//! A failing step ends `clone` with Git's message and status, the workspace attached with
//! what was done, and the steps after it do not run: `git dupe detach --force` and `clone`
//! start over. Neither `URL` nor `BRANCH` begins with `-`: the front reports such a word
//! as a misuse, because each reaches a step as a plain word that Git would read as an
//! option. `clone` never settles: the sequence does, after it, wherever it ends attached.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use super::init::{self, NotAttached};
use super::refusals::{self, Refusal};
use super::settings::Unresolved;
use super::step;
use super::write::{self, Written};
use crate::guards::places::{Place, Places};
use crate::keeper::Failed;
use crate::runner::Run;
use crate::runner::locate::Workspace;
use crate::runner::records;

/// How `clone` attached.
pub enum Cloned {
    /// The remote has no branch: `origin` is configured, and nothing is checked out.
    NoBranch,
    /// The branch is checked out, with what the write step kept.
    CheckedOut(Written),
}

pub enum NotCloned {
    /// The workspace was attached before the command; nothing ran that writes.
    Attached,
    /// G4 refuses the workspace; nothing ran that writes.
    Refused(Refusal),
    /// This word names this public place; nothing ran that writes.
    NamesAPublicPlace(Vec<u8>, Place),
    /// Something that is not a directory stands at the private Git directory's path;
    /// nothing ran that writes.
    Obstructed(PathBuf),
    /// The relative work tree cannot be computed; nothing ran that writes.
    Unresolved(Unresolved),
    /// No `BRANCH` was given, and the remote's `HEAD` names no branch it has; the
    /// workspace is attached with `origin` configured and fetched.
    NoDefaultBranch,
    /// A Git step failed, and the steps after it did not run.
    Failed(Failed),
}

impl From<Failed> for NotCloned {
    fn from(failed: Failed) -> Self {
        NotCloned::Failed(failed)
    }
}

impl From<NotAttached> for NotCloned {
    fn from(not_attached: NotAttached) -> Self {
        match not_attached {
            NotAttached::Obstructed(path) => NotCloned::Obstructed(path),
            NotAttached::Unresolved(unresolved) => NotCloned::Unresolved(unresolved),
            NotAttached::Failed(failed) => NotCloned::Failed(failed),
        }
    }
}

/// Attaches the workspace from the private repository at `url`, checking out `branch`
/// when one is given, else the remote's default branch. `words` are every word after
/// `clone`, each asked of `places`, the project's public places.
pub fn clone(
    workspace: &Workspace,
    places: &Places,
    words: &[OsString],
    url: &OsStr,
    branch: Option<&OsStr>,
) -> Result<Cloned, NotCloned> {
    if workspace.attached() {
        return Err(NotCloned::Attached);
    }
    if let Some(refusal) = refusals::refusal(workspace)? {
        return Err(NotCloned::Refused(refusal));
    }
    for word in words {
        if let Some(place) = places.named_by(word.as_bytes()) {
            return Err(NotCloned::NamesAPublicPlace(
                word.as_bytes().to_vec(),
                place.clone(),
            ));
        }
    }

    init::attach(workspace, None)?;
    let private = workspace.private_directory();
    let root = workspace.root();
    let run = |words: &[&OsStr]| Run::private(&private, root, words).from(root);
    let quiet = |words: &[&OsStr]| step(run(words).capture_output(), &[0]);
    let word = OsStr::new;

    quiet(&[word("remote"), word("add"), word("origin"), url])?;
    step(run(&[word("fetch"), word("origin")]), &[0])?;
    let branches = quiet(&[
        word("for-each-ref"),
        word("--format=%(refname)"),
        word("refs/remotes/origin/"),
    ])?;
    if branches.stdout.is_empty() {
        return Ok(Cloned::NoBranch);
    }
    let branch = match branch {
        Some(branch) => branch.to_owned(),
        None => {
            let answer = quiet(&[
                word("ls-remote"),
                word("--symref"),
                word("origin"),
                word("HEAD"),
            ])?;
            match records::default_branch(&answer.stdout) {
                Some(named) => OsString::from_vec(named.to_vec()),
                None => return Err(NotCloned::NoDefaultBranch),
            }
        }
    };
    // The start point is spelled whole, so that a tag the fetch brought that shares its
    // short name cannot make it ambiguous, and `core.warnAmbiguousRefs` is off, so that
    // Git takes the first ref its rules find, the whole name itself, and a tag spelled as
    // that whole name (`refs/tags/refs/remotes/origin/BRANCH`) cannot either. `--` keeps a
    // default branch whose name begins with `-` from being read as an option: Git then
    // refuses it as a branch name, where `--list` would otherwise end the step with
    // nothing created.
    let tracked = joined(b"refs/remotes/origin/", &branch);
    let head = joined(b"refs/heads/", &branch);
    quiet(&[
        word("-c"),
        word("core.warnAmbiguousRefs=false"),
        word("branch"),
        word("--track"),
        word("--"),
        &branch,
        &tracked,
    ])?;
    quiet(&[word("symbolic-ref"), word("HEAD"), &head])?;
    quiet(&[word("reset"), word("-q")])?;
    Ok(Cloned::CheckedOut(write::write(&private, root)?))
}

/// `prefix` then `branch`, as one word.
fn joined(prefix: &[u8], branch: &OsStr) -> OsString {
    OsString::from_vec([prefix, branch.as_bytes()].concat())
}
