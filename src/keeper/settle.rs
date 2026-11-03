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
//! guards decide over the same two listings, `.gitdupe` being a region path always: no
//! further Git run asks (G9, G11, G22, G25, G27).
//!
//! A `.gitdupe` that cannot be read, and a private listing that settle cannot take, leave
//! the region as it is, because without either the hidden paths are not known and a rule
//! would be dropped, what it hid then reaching public Git (F5, G6). The public
//! listing comes after the replacement, because its query holds the foreign paths of the
//! file as replaced; one that cannot be taken is one warning that exposure was not
//! checked, the region as replaced (`Composition/Keeper`). The command keeps its status
//! either way, except that a list that does not fit on one command line is refused naming
//! its count (G23).

use std::collections::{BTreeSet, HashMap};
use std::io;

use super::exposure::{self, Asked};
use super::foreign::Foreign;
use super::gitdupe::{self, SetAside};
use super::hidden::{self, HiddenPaths};
use super::region::{self, StartingRegion};
use super::{Failed, listing};
use crate::guards::hide as decision;
use crate::guards::operand::{self, Cleaned};
use crate::runner::locate::{Workspace, present};

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
    let listed = gitdupe::read(workspace);
    if let Some(cause) = listed.unreadable() {
        return Settled {
            warnings: vec![unreadable_line(cause)],
            refused: None,
        };
    }
    let mut warnings: Vec<Vec<u8>> = listed.set_aside.iter().map(set_aside_line).collect();
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
        .filter(|path| present(root, path))
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

    let query = query(&hidden, &released, &standing);
    let publicly_tracked = match listing::public(workspace, &query) {
        Ok(tracked) => tracked,
        Err(failed) => return stopped(warnings, failed, Listing::Public),
    };

    let listings = Listings {
        hidden: &hidden,
        publicly_tracked: &publicly_tracked,
    };
    let asked = listings.asked(&released, &standing, &foreign);
    // A question that failed, a signal included, is its warning alone: the command keeps
    // its status (`Composition/Keeper`).
    let answered = exposure::warnings(workspace, &asked, &publicly_tracked, &mut known);
    warnings.extend(hidden.unnamed.iter().map(|path| hidden::unnamed_line(path)));
    warnings.extend(answered.warnings);
    let refused = answered.refused;

    for path in publicly_tracked.intersection(&hidden.privately_tracked) {
        warnings.push(tracked_by_both_line(path));
    }
    Settled { warnings, refused }
}

/// The public listing's query: the region paths, the released paths, and the foreign paths
/// standing here. A released or a foreign path is present by `lstat`, and so a name; a
/// region path that is none stays off this command line, which would otherwise refuse the
/// whole command (G30).
fn query(hidden: &HiddenPaths, released: &[&[u8]], standing: &[&[u8]]) -> Vec<Vec<u8>> {
    hidden
        .region
        .iter()
        .filter(|path| hidden.named(path))
        .cloned()
        .chain(released.iter().map(|path| path.to_vec()))
        .chain(standing.iter().map(|path| path.to_vec()))
        .collect()
}

/// What settle's two listings said, which each path it asks about is read against.
struct Listings<'l> {
    hidden: &'l HiddenPaths,
    publicly_tracked: &'l BTreeSet<Vec<u8>>,
}

impl<'l> Listings<'l> {
    /// The paths the question asks about, in order, each with its kind: every hidden path
    /// but one that is no name, which `settle` names instead (G30), tracked by both where the public listing names a privately tracked one (G8); every
    /// released path; every foreign path standing here; the last two with whether `hide`
    /// would take them and the worktrees whose regions hold them (G9, G27).
    fn asked<'o>(
        &self,
        released: &[&'l [u8]],
        standing: &[&'l [u8]],
        foreign: &Foreign<'o>,
    ) -> Vec<(&'l [u8], Asked<'o>)> {
        let named = self
            .hidden
            .hidden
            .iter()
            .filter(|path| self.hidden.named(path));
        let hidden = named.map(|path| {
            let both = self.hidden.privately_tracked.contains(path)
                && self.publicly_tracked.contains(path);
            let kind = if both {
                Asked::TrackedByBoth
            } else {
                Asked::Hidden
            };
            (path.as_slice(), kind)
        });
        let released = released.iter().map(|path| {
            let released = Asked::Released {
                hideable: self.hideable(path),
                owners: foreign.holding(path),
            };
            (*path, released)
        });
        let standing = standing.iter().map(|path| {
            let foreign = Asked::Foreign {
                hideable: self.hideable(path),
                owners: foreign.holding(path),
            };
            (*path, foreign)
        });
        hidden.chain(released).chain(standing).collect()
    }

    /// Whether `git dupe hide -- <path>`, run from the root, would hide the released `path`
    /// again: a path whose offered operand `hide` reads as that literal path
    /// (`operand::offered`), and none of `hide`'s refusals over the listings settle took,
    /// which hold the path and `.gitdupe` (F5, G11).
    fn hideable(&self, path: &[u8]) -> bool {
        operand::literal(&operand::offered(path)).is_ok()
            && decision::refusal(
                &[path.to_vec()],
                gitdupe::NAME,
                &self.hidden.privately_tracked,
                self.publicly_tracked,
            )
            .is_none()
    }
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

fn unreadable_line(cause: &io::Error) -> Vec<u8> {
    [
        b".gitdupe cannot be read as a file (",
        cause.to_string().as_bytes(),
        b"); the managed region is left as it is and exposure was not checked",
    ]
    .concat()
}

fn tracked_by_both_line(path: &[u8]) -> Vec<u8> {
    [
        path,
        b" is tracked by both the public and the private repository; \
          'git rm --cached' or 'git dupe rm --cached' releases it from one",
    ]
    .concat()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::names_every_byte_string;

    #[test]
    fn a_hidden_path_that_is_no_name_is_kept_off_the_query_and_the_question_and_named() {
        let hidden = HiddenPaths::of(
            vec![b"notes".to_vec(), b"a/\xff".to_vec()],
            BTreeSet::from([b"x/y".to_vec()]),
        );
        let released: [&[u8]; 1] = [b"gone"];
        let standing: [&[u8]; 1] = [b"theirs"];
        let query = query(&hidden, &released, &standing);
        let tracked = BTreeSet::new();
        let listings = Listings {
            hidden: &hidden,
            publicly_tracked: &tracked,
        };
        let others = Vec::new();
        let foreign = Foreign::of(&others);
        let asked = listings.asked(&released, &standing, &foreign);
        let asked: Vec<&[u8]> = asked.iter().map(|(path, _)| *path).collect();

        let mut named: Vec<&[u8]> = vec![b".gitdupe", b"notes", b"x/y"];
        if names_every_byte_string() {
            named.insert(1, b"a/\xff");
            assert!(hidden.unnamed.is_empty());
        } else {
            assert_eq!(hidden.unnamed, [b"a/\xff"]);
        }
        assert_eq!(asked, [&named[..], &released, &standing].concat());
        let queried: Vec<&[u8]> = query.iter().map(Vec::as_slice).collect();
        assert_eq!(queried, [&named[..], &released, &standing].concat());
        // Its rule stands either way: bytes in a file.
        assert!(hidden.region.iter().any(|path| path == b"a/\xff"));
    }
}
