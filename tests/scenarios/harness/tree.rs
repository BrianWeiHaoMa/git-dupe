//! A directory tree as it stands, to compare before and after a command that must change
//! nothing, or nothing at some paths: every entry below it by name, kind, permissions, and
//! content.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(PartialEq, Eq)]
enum Entry {
    Directory { mode: u32 },
    File { mode: u32, content: Vec<u8> },
    Link { target: PathBuf },
}

pub struct Tree {
    entries: BTreeMap<PathBuf, Entry>,
}

impl Tree {
    /// Reads everything below `directory`, a repository's `.git` included.
    pub fn of(directory: &Path) -> Tree {
        let mut entries = BTreeMap::new();
        read_into(&mut entries, directory);
        Tree { entries }
    }

    /// Everything below `directory`, each entry by its path below it, so that the trees of
    /// two copies of one directory compare.
    pub fn relative(directory: &Path) -> Tree {
        let entries = Tree::of(directory)
            .entries
            .into_iter()
            .map(|(path, entry)| (path.strip_prefix(directory).unwrap().to_path_buf(), entry))
            .collect();
        Tree { entries }
    }

    /// The working tree of the workspace at `dir`: everything below it but its `.git`.
    pub fn working(dir: &Path) -> Tree {
        Tree::of(dir).without(&[&dir.join(".git")])
    }

    /// The same tree without the entries at or below each of `paths`: for a comparison
    /// that leaves out what a command may change.
    pub fn without(mut self, paths: &[&Path]) -> Tree {
        self.entries
            .retain(|path, _| !paths.iter().any(|left_out| path.starts_with(left_out)));
        self
    }

    /// The paths that are not the same in `later`: added, removed, or different in kind,
    /// permissions, or content. Empty when nothing changed.
    pub fn changed_in(&self, later: &Tree) -> Vec<PathBuf> {
        let mut changed: Vec<PathBuf> = self
            .differing_from(later)
            .chain(later.differing_from(self))
            .cloned()
            .collect();
        changed.sort();
        changed.dedup();
        changed
    }

    /// The paths at which this tree holds what neither `one` nor `other` holds, nothing at
    /// a path counting as what it holds there: for a tree that must, path by path, be as one
    /// of two others.
    pub fn as_neither(&self, one: &Tree, other: &Tree) -> Vec<PathBuf> {
        let paths: BTreeSet<&PathBuf> = [self, one, other]
            .into_iter()
            .flat_map(|tree| tree.entries.keys())
            .collect();
        paths
            .into_iter()
            .filter(|path| {
                let here = self.entries.get(*path);
                here != one.entries.get(*path) && here != other.entries.get(*path)
            })
            .cloned()
            .collect()
    }

    /// Every path read, in order.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.entries.keys().map(PathBuf::as_path)
    }

    /// Whether an entry stood at `path` when the tree was read.
    pub fn holds(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    /// Whether `path` and everything below it are the same in `later`: nothing added,
    /// removed, or different there, and nothing at either when nothing stood at `path`.
    pub fn same_at_or_below(&self, later: &Tree, path: &Path) -> bool {
        self.at_or_below(path).eq(later.at_or_below(path))
    }

    fn at_or_below<'t>(&'t self, path: &'t Path) -> impl Iterator<Item = (&'t PathBuf, &'t Entry)> {
        self.entries
            .iter()
            .filter(move |(at, _)| at.starts_with(path))
    }

    /// The paths here that `other` lacks or holds differently.
    fn differing_from<'t>(&'t self, other: &'t Tree) -> impl Iterator<Item = &'t PathBuf> {
        self.entries
            .iter()
            .filter(|(path, entry)| other.entries.get(*path) != Some(entry))
            .map(|(path, _)| path)
    }
}

/// Nothing below `dir` changed since `before` was read there.
pub fn unchanged(before: &Tree, dir: &Path) {
    let changed = before.changed_in(&Tree::of(dir));
    assert!(changed.is_empty(), "changed: {changed:?}");
}

/// The paths of the working tree at `dir`, relative to it, added, removed, or different
/// since `before` was read there by `Tree::working`, in order.
pub fn changed_since(before: &Tree, dir: &Path) -> Vec<String> {
    before
        .changed_in(&Tree::working(dir))
        .iter()
        .map(|path| {
            let relative = path.strip_prefix(dir).unwrap();
            relative.to_str().unwrap().to_owned()
        })
        .collect()
}

fn read_into(entries: &mut BTreeMap<PathBuf, Entry>, directory: &Path) {
    let listing =
        fs::read_dir(directory).unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()));
    for entry in listing {
        let path = entry
            .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()))
            .path();
        let failed = |cause: std::io::Error| -> ! { panic!("{}: {cause}", path.display()) };
        let metadata = fs::symlink_metadata(&path).unwrap_or_else(|cause| failed(cause));
        let mode = metadata.permissions().mode();
        let entry = if metadata.is_symlink() {
            Entry::Link {
                target: fs::read_link(&path).unwrap_or_else(|cause| failed(cause)),
            }
        } else if metadata.is_dir() {
            read_into(entries, &path);
            Entry::Directory { mode }
        } else {
            Entry::File {
                mode,
                content: fs::read(&path).unwrap_or_else(|cause| failed(cause)),
            }
        };
        entries.insert(path, entry);
    }
}
