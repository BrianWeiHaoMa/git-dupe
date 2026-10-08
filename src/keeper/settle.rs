//! Settle: what every command G6 covers ends with (R3).
//!
//! In this order: the hidden paths, from `.gitdupe` and one private listing; the region
//! replaced with one rule per region path, a stale region left out, which hands back the
//! other worktrees' regions as the file was read to be written where it maintained the
//! region (`region`, `foreign`); the foreign paths, from those regions alone; one public
//! listing under the region paths, the released paths, and the foreign paths; one
//! exposure question about every hidden path, every released path, and every foreign
//! path, a path the listing names being one public Git does not ignore. Each finding is
//! one warning. A foreign path is named only where public Git ignores it and the listing
//! does not name it, and never where the region is left as it is (G27). A released or
//! foreign path's warning names `git dupe hide` when `hide` would take it, which the
//! guards decide over the same two listings, `.gitdupe` being a region path always, and
//! only while `.gitdupe` can be read as a file, which `hide` rewrites: no further Git run
//! asks (G9, G11, G22, G25, G27).
//!
//! The private listing that settle cannot take leaves the region as it is, because
//! without it the hidden paths are not known and a rule would be dropped. The public
//! listing comes after the replacement, because its query holds the foreign paths of the
//! file as replaced; one that cannot be taken is one warning that exposure was not
//! checked, the region as replaced (`Composition/Keeper`). The command keeps its status
//! either way, except that a list that does not fit on one command line is refused naming
//! its count (G23).

use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use super::exposure::{self, Asked};
use super::foreign::Foreign;
use super::gitdupe::{self, SetAside, Source};
use super::hidden::HiddenPaths;
use super::region::{self, StartingRegion};
use super::{Failed, listing};
use crate::guards::hide as decision;
use crate::guards::operand::{self, Cleaned};
use crate::runner::locate::Workspace;

/// What settle leaves for the front: its warnings, in order, and the count of a list
/// that did not fit, which refuses the command.
pub struct Settled {
    pub warnings: Vec<Vec<u8>>,
    pub refused: Option<usize>,
}

/// Which listing a failure stopped.
enum Listing {
    Private,
    Public,
}

pub fn settle(workspace: &Workspace, starting: &StartingRegion) -> Settled {
    let mut warnings = Vec::new();
    let listed = gitdupe::read(workspace);
    for set_aside in &listed.set_aside {
        warnings.push(set_aside_line(set_aside));
    }
    if let Source::Unreadable(cause) = &listed.source {
        warnings.push(
            [
                b".gitdupe cannot be read as a file (",
                cause.to_string().as_bytes(),
                b") and hides nothing",
            ]
            .concat(),
        );
    }
    let privately_tracked = match listing::private(workspace) {
        Ok(tracked) => tracked,
        Err(failed) => return stopped(warnings, failed, Listing::Private),
    };
    let hidden = HiddenPaths::of(listed.paths, privately_tracked);
    let root = workspace.root();
    // Released: in the starting region, neither hidden now nor below a hidden path, and
    // present by `lstat` (G9).
    let released: Vec<&[u8]> = starting
        .paths
        .iter()
        .map(Vec::as_slice)
        .filter(|path| !hidden.hides(path))
        .filter(|path| fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_ok())
        .collect();

    let replaced = region::replace(workspace, &hidden.region);
    warnings.extend(replaced.warnings);
    // A region left as it is names no foreign path (G27).
    let others = replaced.others.unwrap_or_default();
    let foreign = Foreign::of(&others);
    // The ancestors looked at for a link, for the foreign paths and then for the hidden
    // and released paths of the question, each once.
    let mut known = HashMap::new();
    let released_set: BTreeSet<&[u8]> = released.iter().copied().collect();
    let standing = foreign.standing(root, &hidden, &released_set, |path| {
        exposure::beyond_a_link(root, path, &mut known)
    });

    let query: Vec<Vec<u8>> = hidden
        .region
        .iter()
        .cloned()
        .chain(released.iter().map(|path| path.to_vec()))
        .chain(standing.iter().map(|path| path.to_vec()))
        .collect();
    let publicly_tracked = match listing::public(workspace, &query) {
        Ok(tracked) => tracked,
        Err(failed) => return stopped(warnings, failed, Listing::Public),
    };

    let hideable = |path: &[u8]| {
        hideable(
            path,
            &listed.source,
            &hidden.privately_tracked,
            &publicly_tracked,
        )
    };
    let asked: Vec<(&[u8], Asked)> = hidden
        .hidden
        .iter()
        .map(|path| {
            let both = hidden.privately_tracked.contains(path) && publicly_tracked.contains(path);
            let kind = if both {
                Asked::TrackedByBoth
            } else {
                Asked::Hidden
            };
            (path.as_slice(), kind)
        })
        .chain(released.iter().map(|path| {
            let released = Asked::Released {
                hideable: hideable(path),
                owners: foreign.holding(path),
            };
            (*path, released)
        }))
        .chain(standing.iter().map(|path| {
            let foreign = Asked::Foreign {
                hideable: hideable(path),
                owners: foreign.holding(path),
            };
            (*path, foreign)
        }))
        .collect();
    // A question that failed, a signal included, is its warning alone: the command keeps
    // its status (`Composition/Keeper`).
    let answered = exposure::warnings(workspace, &asked, &publicly_tracked, &mut known);
    warnings.extend(answered.warnings);
    let refused = answered.refused;

    for path in publicly_tracked.intersection(&hidden.privately_tracked) {
        warnings.push(
            [
                &path[..],
                b" is tracked by both the public and the private repository; \
                  'git rm --cached' or 'git dupe rm --cached' releases it from one",
            ]
            .concat(),
        );
    }
    Settled { warnings, refused }
}

/// Whether `git dupe hide -- <path>`, run from the root, would hide the released `path`
/// again: a word `hide` reads as a literal path, a `.gitdupe` the keeper can rewrite,
/// which one that cannot be read as a file is not (`edit`), and none of `hide`'s refusals
/// over the listings settle took, which hold the path and `.gitdupe` (F5, G11).
fn hideable(
    path: &[u8],
    gitdupe: &Source,
    privately_tracked: &BTreeSet<Vec<u8>>,
    publicly_tracked: &BTreeSet<Vec<u8>>,
) -> bool {
    operand::literal(path).is_ok()
        && !matches!(gitdupe, Source::Unreadable(_))
        && decision::refusal(
            &[path.to_vec()],
            gitdupe::NAME,
            privately_tracked,
            publicly_tracked,
        )
        .is_none()
}

/// Settle stopped at a listing: the private one before the replacement, the region left
/// as it is, or the public one after it, the region as replaced.
fn stopped(mut warnings: Vec<Vec<u8>>, failed: Failed, listing: Listing) -> Settled {
    if let Failed::TooLong(count) = failed {
        return Settled {
            warnings,
            refused: Some(count),
        };
    }
    let line = match listing {
        Listing::Private => [
            b"cannot read the private repository (",
            &failed.cause()[..],
            b"); the managed region is left as it is and exposure was not checked; \
              run 'git dupe init' to complete the private repository",
        ]
        .concat(),
        Listing::Public => [
            b"cannot list what public Git tracks under the hidden paths (",
            &failed.cause()[..],
            b"); the managed region is replaced, but exposure was not checked",
        ]
        .concat(),
    };
    warnings.push(line);
    Settled {
        warnings,
        refused: None,
    }
}

fn set_aside_line(set_aside: &SetAside) -> Vec<u8> {
    let number = set_aside.line.to_string();
    let (what, path): (&[u8], &[u8]) = match &set_aside.cleaned {
        Cleaned::Root => (b"the root", b""),
        Cleaned::Outside(path) => (b"a path outside the root: ", path),
        Cleaned::Inside(path) => (b"", path),
    };
    [
        b".gitdupe line ",
        number.as_bytes(),
        b" names ",
        what,
        path,
        b", and hides nothing",
    ]
    .concat()
}
