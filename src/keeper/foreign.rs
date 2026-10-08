//! The other worktrees' regions as the one composition read them, and the paths they hide
//! that stand here (G3, G27, `Composition/Keeper`).
//!
//! `region`'s update hands back, beside what it did, every other worktree's region it kept
//! — those whose worktree held a private repository when the file was read to be written —
//! each with its worktree and its rules read back into paths. That is the one observation
//! of the other regions a command uses, after the lock is let go: no second read of the
//! file decides what is named (R10).
//!
//! A **foreign path** is a path of one of them that this worktree hides neither itself
//! nor through an ancestor, that lies beyond no symbolic link here, and at which `lstat`
//! finds something under the root: settle asks the one exposure question about it and
//! names it when public Git ignores it, by whichever rule, and does not track it (G27).
//! It is not made a hidden path here: it joins no region, no listing of a handler, and no
//! staging. Equal paths of several regions are one path with every worktree whose region
//! holds it; a path is never dropped because another region holds its parent, because
//! both are paths a region hides. `detach` instead attributes each formerly hidden path it
//! asks about to every region holding that path or an ancestor of it (G3). The
//! comparisons are `guards::operand`'s lexical ones over bytes (R9); Git alone says what is
//! ignored.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::iter;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::hidden::HiddenPaths;
use crate::guards::operand;
use crate::guards::quoted::shell_word;

/// Whose region another region is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    Main,
    /// The linked worktree of this name, as bytes.
    Linked(Vec<u8>),
}

/// Another worktree's region, as the composition kept it: its worktree, and the paths its
/// rules were written for.
#[derive(Debug, PartialEq, Eq)]
pub struct Region {
    pub owner: Owner,
    pub paths: Vec<Vec<u8>>,
}

/// Every path of the other regions, each once, with every worktree whose region holds it,
/// in the file's order.
pub struct Foreign<'r> {
    owners: BTreeMap<&'r [u8], Vec<&'r Owner>>,
}

impl<'r> Foreign<'r> {
    pub fn of(regions: &'r [Region]) -> Foreign<'r> {
        let mut owners: BTreeMap<&[u8], Vec<&Owner>> = BTreeMap::new();
        for region in regions {
            for path in &region.paths {
                let holding = owners.entry(path).or_default();
                if !holding.contains(&&region.owner) {
                    holding.push(&region.owner);
                }
            }
        }
        Foreign { owners }
    }

    /// The worktrees whose regions hold exactly `path`.
    pub fn holding(&self, path: &[u8]) -> Vec<&'r Owner> {
        self.owners.get(path).cloned().unwrap_or_default()
    }

    /// The worktrees whose regions hold `path` or an ancestor of it, each once, those of
    /// the outermost path first.
    pub fn hiding(&self, path: &[u8]) -> Vec<&'r Owner> {
        let mut hiding: Vec<&Owner> = Vec::new();
        for at in operand::ancestors(path).chain(iter::once(path)) {
            for owner in self.owners.get(at).into_iter().flatten() {
                if !hiding.contains(owner) {
                    hiding.push(owner);
                }
            }
        }
        hiding
    }

    /// The foreign paths, in byte order, but those among `released`, which the caller
    /// asks about already and whose owners it takes from `holding`. A path this worktree
    /// hides is not looked at; `beyond_a_link` says, by `lstat` of its ancestors outermost
    /// first, whether a symbolic link lies above the others, and only a path with none
    /// above it is itself looked at, so that nothing beyond a link is touched. One `lstat`
    /// per path at most, and the caller's `beyond_a_link` keeps its answers for an
    /// ancestor already looked at (`Holds/G27`).
    pub fn standing(
        &self,
        root: &Path,
        hidden: &HiddenPaths,
        released: &BTreeSet<&[u8]>,
        mut beyond_a_link: impl FnMut(&[u8]) -> bool,
    ) -> Vec<&'r [u8]> {
        self.owners
            .keys()
            .copied()
            .filter(|path| !hidden.hides(path) && !released.contains(path))
            .filter(|path| !beyond_a_link(path))
            .filter(|path| fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_ok())
            .collect()
    }
}

/// The warning for a path that stands here, that public Git ignores and does not track,
/// and that the regions of `owners` hide, itself or through an ancestor, while this
/// worktree does not: settle's foreign or released path (G27), and a path `detach` hid
/// (G3). `hideable`, which only settle can say, names `git dupe hide` where it would take
/// the path here.
pub fn still_hidden(path: &[u8], owners: &[&Owner], hideable: bool) -> Vec<u8> {
    let mut line = [
        path,
        b" stands here and is hidden by ",
        &named(owners),
        b" alone: public Git ignores it here, and this worktree does not hide it",
    ]
    .concat();
    if hideable {
        // Root-relative, as the released path's own remedy is; `--` keeps a path
        // beginning with `-` a path, and the shell takes it as one word.
        line.extend_from_slice(b"; run from the root, 'git dupe hide -- ");
        line.extend_from_slice(&shell_word(&operand::offered(path)));
        line.extend_from_slice(b"' hides it here too");
    }
    line
}

/// The worktrees as a line names them: `the main worktree` or `worktree <name>`, the name's
/// bytes as they stand, joined with commas and a final `and`.
fn named(owners: &[&Owner]) -> Vec<u8> {
    let names: Vec<Vec<u8>> = owners
        .iter()
        .map(|owner| match owner {
            Owner::Main => b"the main worktree".to_vec(),
            Owner::Linked(name) => [&b"worktree "[..], name].concat(),
        })
        .collect();
    match names.as_slice() {
        [] => Vec::new(),
        [one] => one.clone(),
        [first, second] => [&first[..], b" and ", second].concat(),
        [rest @ .., last] => [&rest.join(&b", "[..])[..], b", and ", last].concat(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(owner: Owner, paths: &[&[u8]]) -> Region {
        Region {
            owner,
            paths: paths.iter().map(|path| path.to_vec()).collect(),
        }
    }

    fn linked(name: &[u8]) -> Owner {
        Owner::Linked(name.to_vec())
    }

    #[test]
    fn equal_paths_of_several_regions_are_one_path_with_every_owner() {
        let regions = [
            region(Owner::Main, &[b".gitdupe", b"notes"]),
            region(linked(b"agent"), &[b".gitdupe", b"notes", b"notes/x"]),
            // A second region of the same worktree adds it once.
            region(Owner::Main, &[b"notes"]),
        ];
        let foreign = Foreign::of(&regions);
        assert_eq!(foreign.holding(b"notes"), [&Owner::Main, &linked(b"agent")]);
        assert_eq!(foreign.holding(b"notes/x"), [&linked(b"agent")]);
        assert!(foreign.holding(b"other").is_empty());
    }

    #[test]
    fn a_path_is_hidden_by_the_regions_holding_it_or_an_ancestor_by_whole_components() {
        let regions = [
            region(Owner::Main, &[b"notes"]),
            region(linked(b"agent"), &[b"notes/x.md", b"notes-old"]),
        ];
        let foreign = Foreign::of(&regions);
        assert_eq!(foreign.hiding(b"notes"), [&Owner::Main]);
        assert_eq!(
            foreign.hiding(b"notes/x.md"),
            [&Owner::Main, &linked(b"agent")]
        );
        assert_eq!(
            foreign.hiding(b"notes/x.md/deeper"),
            foreign.hiding(b"notes/x.md")
        );
        // A byte-prefix sibling is not below `notes`.
        assert_eq!(foreign.hiding(b"notes-old"), [&linked(b"agent")]);
        assert!(foreign.hiding(b"notesx").is_empty());
        assert!(foreign.hiding(b"note").is_empty());
    }

    #[test]
    fn the_line_names_every_owner_and_hide_only_where_it_would_take_the_path() {
        let agent = linked(b"caf\xe9");
        let other = linked(b"b");
        assert_eq!(
            still_hidden(b"notes", &[&Owner::Main], false),
            b"notes stands here and is hidden by the main worktree alone: public Git ignores \
              it here, and this worktree does not hide it"
        );
        assert_eq!(
            still_hidden(b"-x", &[&Owner::Main, &agent], true),
            b"-x stands here and is hidden by the main worktree and worktree caf\xe9 alone: \
              public Git ignores it here, and this worktree does not hide it; run from the \
              root, 'git dupe hide -- -x' hides it here too"
        );
        assert!(
            still_hidden(b"n", &[&Owner::Main, &agent, &other], false).starts_with(
                b"n stands here and is hidden by the main worktree, worktree caf\xe9, and \
                  worktree b alone:"
            )
        );
    }

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_foreign_path_stands_here_beyond_no_link_and_is_not_hidden_here() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-foreign-{}", std::process::id())));
        let _ = fs::remove_dir_all(&scratch.0);
        let root = scratch.0.join("root");
        for directory in ["notes", "mine", "kept/sub", "elsewhere/inner"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join("link")).unwrap();
        std::os::unix::fs::symlink(root.join("nothing"), root.join("dangling")).unwrap();
        fs::write(root.join("released"), b"").unwrap();
        let regions = [region(
            Owner::Main,
            &[
                b".gitdupe",
                b"notes",
                b"mine/x",
                b"absent",
                b"link/inner",
                b"link",
                b"dangling",
                b"kept",
                b"kept/sub",
                b"released",
            ],
        )];
        let foreign = Foreign::of(&regions);
        let hidden = HiddenPaths::of(vec![b"mine".to_vec()], BTreeSet::new());
        let released = BTreeSet::from([&b"released"[..]]);
        let mut looked_at = Vec::new();
        let standing = foreign.standing(&root, &hidden, &released, |path| {
            looked_at.push(path.to_vec());
            operand::ancestors(path).any(|above| above == b"link")
        });
        // `.gitdupe` and `mine/x` are hidden here, `absent` stands nowhere, `link/inner`
        // lies beyond a link; a link at the path itself, a dangling one included, stands
        // here; a path is kept beside its foreign parent.
        assert_eq!(
            standing,
            [&b"dangling"[..], b"kept", b"kept/sub", b"link", b"notes"]
        );
        // Hidden and released paths are not looked at.
        assert!(!looked_at.contains(&b"mine/x".to_vec()));
        assert!(!looked_at.contains(&b".gitdupe".to_vec()));
        assert!(!looked_at.contains(&b"released".to_vec()));
    }
}
