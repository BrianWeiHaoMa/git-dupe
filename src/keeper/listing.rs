//! The listings: the files the private index holds, the paths whose deletion it has
//! staged, and the paths the public index, or the index the user's own command reads,
//! holds under given query paths. Each is one run, `ls-files` or `diff --cached`, its
//! paths root-relative in the `-z` form, whatever directory the command was typed in.

use std::collections::BTreeSet;

use super::Failed;
use crate::guards::pathspec;
use crate::runner::locate::Workspace;
use crate::runner::records;
use crate::runner::{End, Failure, Run};

/// Every file the private index holds, from one private `ls-files` from the root.
pub fn private(workspace: &Workspace) -> Result<BTreeSet<Vec<u8>>, Failed> {
    let directory = workspace.private_directory();
    let run = Run::private(
        &directory,
        workspace.root(),
        ["ls-files", "-z", "--full-name"],
    )
    .from(workspace.root())
    .own_paths()
    .capture_output();
    listed(run, None)
}

/// The paths whose deletion the private index has staged, from one private
/// `diff --cached` from the root (S8): against the empty tree while `HEAD` is unborn, and
/// with no rename pairing a deletion with an addition.
pub fn staged_deletions(workspace: &Workspace) -> Result<BTreeSet<Vec<u8>>, Failed> {
    let directory = workspace.private_directory();
    let words = [
        "diff",
        "--cached",
        "--name-only",
        "-z",
        "--diff-filter=D",
        "--no-renames",
        "--no-relative",
    ];
    let run = Run::private(&directory, workspace.root(), words)
        .from(workspace.root())
        .capture_output();
    listed(run, None)
}

/// The paths the public index holds at or below `query`, root-relative paths each
/// written as `:(top,literal)<path>`, from one public `ls-files` that reads the public
/// index and not a hook's. A list that does not fit on the command line is `TooLong`
/// naming how many paths it carried.
pub fn public(workspace: &Workspace, query: &[Vec<u8>]) -> Result<BTreeSet<Vec<u8>>, Failed> {
    let run = public_listing(query).from(workspace.root());
    listed(run, Some(query.len()))
}

/// The paths at or below `query` that the index the user's own command reads holds: the
/// one a received `GIT_INDEX_FILE` names, as a hook's does (S9), else the public index.
/// The same listing as `public`, run from the user's directory with that variable kept,
/// as the user's command runs, so that a relative value names the same index for both:
/// `clean` decides by it its nested repositories and which directory at a hidden path
/// Git enters (`Holds/G16`).
pub fn users_index(query: &[Vec<u8>]) -> Result<BTreeSet<Vec<u8>>, Failed> {
    listed(
        public_listing(query).in_the_users_index(),
        Some(query.len()),
    )
}

/// One public `ls-files` of the paths at or below `query`, each written
/// `:(top,literal)<path>`, which name the same paths from any directory (S4).
fn public_listing<'r>(query: &[Vec<u8>]) -> Run<'r> {
    let words = ["ls-files", "-z", "--full-name", "--"]
        .map(Into::into)
        .into_iter()
        .chain(query.iter().map(|path| pathspec::top_literal(path)));
    Run::public(words).own_paths().capture_output()
}

/// Runs a listing and reads its NUL-terminated paths, each once. `carried` is the count
/// of paths on its command line, when it carries a list.
fn listed(run: Run, carried: Option<usize>) -> Result<BTreeSet<Vec<u8>>, Failed> {
    let finished = run.start().map_err(|failure| match (failure, carried) {
        (Failure::TooLong, Some(count)) => Failed::TooLong(count),
        (failure, _) => Failed::NotStarted(failure),
    })?;
    if finished.end != End::Code(0) {
        return Err(Failed::Exited(finished.end));
    }
    Ok(records::paths(&finished.stdout))
}
