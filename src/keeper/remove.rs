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
//! one public listing under the region paths and the released paths among them names
//! those public Git tracks, which it does not ignore whatever the question answers
//! (`Holds/G3`). A listing that fails is one warning, as a question that fails is, and,
//! as a failed question names no path as still hidden, no path is then named as one
//! public Git ignores because another worktree hides it: whether public Git tracks it,
//! and so sees it, is not known (G7).

use std::collections::{BTreeSet, HashMap};

use super::exposure::{self, Answered, Asked};
use super::foreign::Foreign;
use super::hidden::HiddenPaths;
use super::region::{self, NotDeleted};
use super::{Failed, gitdupe, listing};
use crate::runner::End;
use crate::runner::locate::{Workspace, present};

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
    let left = hidden
        .hidden
        .iter()
        .map(Vec::as_slice)
        .filter(|path| present(workspace.root(), path))
        .map(|path| {
            let owners = foreign.hiding(path);
            (path, Asked::Left { owners })
        });
    let released = deleted
        .paths
        .iter()
        .map(Vec::as_slice)
        .filter(|path| !hidden.hides(path))
        .filter(|path| present(workspace.root(), path))
        // No private repository stands once `detach` ends, so `hide` is not named (G4).
        .map(|path| {
            let released = Asked::Released {
                hideable: false,
                owners: foreign.hiding(path),
            };
            (path, released)
        });
    let mut asked: Vec<(&[u8], Asked)> = left.chain(released).collect();
    let mut warnings: Vec<Vec<u8>> = deleted.warning.into_iter().collect();
    let mut refused = None;
    let mut killed = None;
    let mut listing_failed = false;
    // Under the region paths among the paths asked about, not every hidden path: each
    // hidden path present lies at or below a present region path, so the listing names
    // the same paths while its list stays one built from E8's budget, however many files
    // a hidden directory holds (`Holds/G3`). A released path is a path of the deleted
    // region, below no hidden path, so it is listed itself.
    let region: BTreeSet<&[u8]> = hidden.region.iter().map(Vec::as_slice).collect();
    let query: Vec<Vec<u8>> = asked
        .iter()
        .filter(|(path, kind)| matches!(kind, Asked::Released { .. }) || region.contains(path))
        .map(|(path, _)| path.to_vec())
        .collect();
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
                            b"); a path it tracks is visible to it and may not be named, \
                              and none is named as hidden by another worktree",
                        ]
                        .concat(),
                    );
                    if let Failed::Exited(End::Signal(_)) = failed {
                        killed = Some(failed);
                    }
                    listing_failed = true;
                }
            }
            BTreeSet::new()
        })
    };
    if listing_failed {
        for (_, kind) in &mut asked {
            if let Asked::Left { owners } | Asked::Released { owners, .. } = kind {
                owners.clear();
            }
        }
    }
    let answered = exposure::warnings(workspace, &asked, &tracked, &mut HashMap::new());
    warnings.extend(answered.warnings);
    Ok(Answered {
        warnings,
        visible: answered.visible,
        refused: refused.or(answered.refused),
        killed: killed.or(answered.killed),
    })
}
