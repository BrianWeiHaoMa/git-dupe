//! `init` after its words: the refusals, then the repository, then the settings, each
//! step done only where it is missing (G1, `State` "`init`"). The relative work tree the
//! settings compare and write is computed first, so that a root or Git directory with no
//! canonical form ends the command before anything is written.
//!
//! The private repository is made, completed, and configured only where its path is a
//! directory by `lstat`, or nothing yet: something else standing there is one of the
//! refusals (`refusals`), because Git's `init` and `config` would follow it.
//!
//! Git's `init` runs only when the private Git directory lacks `HEAD`, `objects`, or
//! `refs`, the repository Git cannot open, because on a complete repository it rewrites
//! keys the developer may have edited, `core.worktree` among them (S6). It gets the
//! initial branch and its own template directory, so that completing a killed `init`
//! copies the template files the kill left missing and keeps those it left, a partly
//! copied one included (G1, G21). Its standard output is Git's own line.

use std::fs;
use std::path::Path;

use super::refusals::{self, Refusal};
use super::settings::{self, Unresolved};
use super::step;
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
    /// The relative work tree cannot be computed; nothing ran that writes.
    Unresolved(Unresolved),
    /// A Git step failed, and the steps after it did not run.
    Failed(Failed),
}

impl From<Failed> for NotInitialized {
    fn from(failed: Failed) -> Self {
        NotInitialized::Failed(failed)
    }
}

/// Why the steps `init` and `clone` share did not all run.
pub(super) enum NotAttached {
    /// The relative work tree cannot be computed; nothing ran that writes.
    Unresolved(Unresolved),
    /// A Git step failed, and the steps after it did not run.
    Failed(Failed),
}

impl From<Failed> for NotAttached {
    fn from(failed: Failed) -> Self {
        NotAttached::Failed(failed)
    }
}

impl From<NotAttached> for NotInitialized {
    fn from(not_attached: NotAttached) -> Self {
        match not_attached {
            NotAttached::Unresolved(unresolved) => NotInitialized::Unresolved(unresolved),
            NotAttached::Failed(failed) => NotInitialized::Failed(failed),
        }
    }
}

/// Attaches the workspace, or completes an attachment a killed `init` left undone, with
/// `branch` as the initial branch when Git's `init` runs and one was given.
pub fn init(workspace: &Workspace, branch: Option<&[u8]>) -> Result<Initialized, NotInitialized> {
    if let Some(refusal) = refusals::refusal(workspace)? {
        return Err(NotInitialized::Refused(refusal));
    }
    Ok(attach(workspace, branch)?)
}

/// What `init` does once nothing refuses, and the first step of `clone`: the repository,
/// then the settings, each where it is missing.
pub(super) fn attach(
    workspace: &Workspace,
    branch: Option<&[u8]>,
) -> Result<Initialized, NotAttached> {
    let private = workspace.private_directory();
    let work_tree = settings::relative_work_tree(workspace).map_err(NotAttached::Unresolved)?;
    let repository = !complete(&private);
    if repository {
        make(workspace, &private, branch)?;
    }
    let recorded = settings::recorded(workspace, &private)?;
    let settings = recorded.unfinished(&work_tree);
    if settings {
        settings::set(workspace, &private, &work_tree, &recorded)?;
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
fn make(workspace: &Workspace, private: &Path, branch: Option<&[u8]>) -> Result<(), Failed> {
    let branch = match branch {
        Some(branch) => Some(branch.to_vec()),
        None => public_branch()?,
    };
    let mut words: Vec<Vec<u8>> = vec!["init".into()];
    if let Some(branch) = branch {
        words.push([b"--initial-branch=", &branch[..]].concat());
    }
    step(
        Run::private(private, workspace.root(), words).from(workspace.root()),
        &[0],
    )?;
    Ok(())
}

/// The branch the public repository's `HEAD` names, from one public
/// `symbolic-ref -q HEAD`, which prints nothing and exits 1 when `HEAD` is detached.
fn public_branch() -> Result<Option<Vec<u8>>, Failed> {
    let run = Run::public(["symbolic-ref", "-q", "HEAD"]).capture_output();
    let answer = step(run, &[0, 1])?;
    Ok(records::branch_of(&answer.stdout).map(<[u8]>::to_vec))
}
