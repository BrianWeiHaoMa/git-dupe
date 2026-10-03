//! `init` after its words: the refusals, then the repository, then the settings, each
//! step done only where it is missing (G1, `State` "`init`").
//!
//! Git's `init` runs only when the private Git directory lacks `HEAD`, `objects`, or
//! `refs`, the repository Git cannot open, because on a complete repository it rewrites
//! keys the developer may have edited, `core.worktree` among them (S6). It gets the
//! initial branch, and `--template=` when `HEAD` exists, so that completing a killed
//! `init` copies no sample hook into it. Its standard output is Git's own line.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

use super::refusals::{self, Refusal};
use super::{settings, step};
use crate::keeper::Failed;
use crate::runner::Run;
use crate::runner::locate::Workspace;
use crate::runner::records;

/// What `init` completed.
pub struct Initialized {
    /// Git's `init` ran, to make the repository or complete it.
    pub repository: bool,
    /// The keys were set.
    pub settings: bool,
}

pub enum NotInitialized {
    /// G4 refuses the workspace; nothing ran that writes.
    Refused(Refusal),
    /// A Git step failed, and the steps after it did not run.
    Failed(Failed),
}

impl From<Failed> for NotInitialized {
    fn from(failed: Failed) -> Self {
        NotInitialized::Failed(failed)
    }
}

/// Attaches the workspace, or completes an attachment a killed `init` left undone, with
/// `branch` as the initial branch when Git's `init` runs and one was given.
pub fn init(workspace: &Workspace, branch: Option<&OsStr>) -> Result<Initialized, NotInitialized> {
    if let Some(refusal) = refusals::refusal(workspace)? {
        return Err(NotInitialized::Refused(refusal));
    }
    Ok(attach(workspace, branch)?)
}

/// What `init` does once nothing refuses, and the first step of `clone`: the repository,
/// then the settings, each where it is missing.
pub(super) fn attach(workspace: &Workspace, branch: Option<&OsStr>) -> Result<Initialized, Failed> {
    let private = workspace.private_directory();
    let repository = !complete(&private);
    if repository {
        make(workspace, &private, branch)?;
    }
    let settings = settings::unfinished(workspace, &private)?;
    if settings {
        settings::set(workspace, &private)?;
    }
    Ok(Initialized {
        repository,
        settings,
    })
}

/// Whether `HEAD`, `objects`, and `refs` all exist in the private Git directory.
fn complete(private: &Path) -> bool {
    ["HEAD", "objects", "refs"]
        .iter()
        .all(|entry| exists(&private.join(entry)))
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Git's `init` in a private run, which names the directory through `GIT_DIR`: given as
/// an operand, Git would make a `.git` inside it.
fn make(workspace: &Workspace, private: &Path, branch: Option<&OsStr>) -> Result<(), Failed> {
    let branch = match branch {
        Some(branch) => Some(branch.to_owned()),
        None => public_branch()?,
    };
    let mut words = vec![OsString::from("init")];
    if let Some(branch) = branch {
        let mut option = OsString::from("--initial-branch=");
        option.push(branch);
        words.push(option);
    }
    if exists(&private.join("HEAD")) {
        words.push("--template=".into());
    }
    step(
        Run::private(private, workspace.root(), words).from(workspace.root()),
        &[0],
    )?;
    Ok(())
}

/// The branch the public repository's `HEAD` names, from one public
/// `symbolic-ref -q HEAD`, which prints nothing and exits 1 when `HEAD` is detached.
fn public_branch() -> Result<Option<OsString>, Failed> {
    let run = Run::public(["symbolic-ref", "-q", "HEAD"]).capture_output();
    let answer = step(run, &[0, 1])?;
    Ok(records::branch_of(&answer.stdout).map(|branch| OsString::from_vec(branch.to_vec())))
}
