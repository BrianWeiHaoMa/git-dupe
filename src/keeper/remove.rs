//! Remove: the managed region deleted for `detach`, and what public Git can see once it
//! is gone (G3, G9, `Composition/Keeper`).
//!
//! The region goes first, by the one write `region` makes, and the question follows it,
//! because before the deletion the region's own rules ignore every hidden path. The same
//! write leaves out a stale region and hands back the other worktrees' regions as it read
//! them; each path asked about that public Git still ignores, that its index does not
//! track, and that one of them holds, itself or through an ancestor, is named with the
//! worktrees whose regions hold it, and is not counted as visible (G3, `foreign`). One
//! `check-ignore` asks about each hidden path as it stood that is still present by
//! `lstat`, and about each path of the deleted region that is neither among them nor below
//! one and is present, released as settle releases a path of the starting region (G9);
//! one public listing under the same paths names those public Git tracks, which it does
//! not ignore whatever the question answers (`Holds/G3`). A listing that fails is one
//! warning, as a question that fails is.

use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use super::exposure::{self, Answered, Asked};
use super::foreign::Foreign;
use super::hidden::HiddenPaths;
use super::region::{self, NotDeleted};
use super::{Failed, gitdupe, listing};
use crate::runner::End;
use crate::runner::locate::Workspace;

/// Deletes the region and returns the warnings: that a link at the exclude file became a
/// regular file, then the question's, a path another worktree's region still hides among
/// them. `hidden` is the hidden paths as they stood, or `None` where the private
/// repository could not be listed, which leaves them `.gitdupe`, the paths the file on
/// disk lists, and the paths of the region as it stood (`Holds/G3`).
/// A region that still stands is the error, and nothing was written.
pub fn remove(workspace: &Workspace, hidden: Option<&HiddenPaths>) -> Result<Answered, NotDeleted> {
    let deleted = region::delete(workspace)?;
    let foreign = Foreign::of(&deleted.others);
    let unlisted;
    let hidden = match hidden {
        Some(hidden) => hidden,
        None => {
            let listed = gitdupe::on_disk(workspace).paths;
            unlisted = HiddenPaths::of([listed, deleted.paths.clone()].concat(), BTreeSet::new());
            &unlisted
        }
    };
    let present =
        |path: &&[u8]| fs::symlink_metadata(workspace.root().join(OsStr::from_bytes(path))).is_ok();
    let left = hidden
        .hidden
        .iter()
        .map(Vec::as_slice)
        .filter(present)
        .map(|path| {
            let owners = foreign.hiding(path);
            (path, Asked::Left { owners })
        });
    let released = deleted
        .paths
        .iter()
        .map(Vec::as_slice)
        .filter(|path| !hidden.hides(path))
        .filter(present)
        // No private repository stands once `detach` ends, so `hide` is not named (G4).
        .map(|path| {
            let released = Asked::Released {
                hideable: false,
                owners: foreign.hiding(path),
            };
            (path, released)
        });
    let asked: Vec<(&[u8], Asked)> = left.chain(released).collect();
    let mut warnings: Vec<Vec<u8>> = deleted.warning.into_iter().collect();
    let mut refused = None;
    let mut killed = None;
    let query: Vec<Vec<u8>> = asked.iter().map(|(path, _)| path.to_vec()).collect();
    let tracked = if query.is_empty() {
        BTreeSet::new()
    } else {
        listing::public(workspace, &query).unwrap_or_else(|failed| {
            match failed {
                Failed::TooLong(count) => refused = Some(count),
                failed => {
                    warnings.push(
                        [
                            b"cannot list what public Git tracks under the hidden paths (",
                            &failed.cause()[..],
                            b"); a path it tracks is visible to it and may not be named",
                        ]
                        .concat(),
                    );
                    if let Failed::Exited(End::Signal(_)) = failed {
                        killed = Some(failed);
                    }
                }
            }
            BTreeSet::new()
        })
    };
    let answered = exposure::warnings(workspace, &asked, &tracked, &mut HashMap::new());
    warnings.extend(answered.warnings);
    Ok(Answered {
        warnings,
        visible: answered.visible,
        refused: refused.or(answered.refused),
        killed: killed.or(answered.killed),
    })
}
