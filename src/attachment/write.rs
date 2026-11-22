//! The write step of `clone` (G2, `Holds/G2`): every privately tracked file absent from
//! the working tree is written, except one at a path the project tracks, and nothing that
//! is there is touched.
//!
//! The order is the guarantee. First the hidden paths, from the keeper, and one public
//! listing under them, which says which of `clone`'s paths the project tracks, the absent
//! ones and the kept ones alike, before anything is written. Its query is the region
//! paths, each replaced by its outermost proper ancestor that exists and is not a
//! directory, by `lstat`, where one stands: never more paths or bytes than settle's region
//! paths (E8, G23), and every path a line could offer to restore lies at or below one of
//! them, so that the one listing answers both questions asked of it: for a path to write,
//! whether the project tracks that path; for a path a restore would name, whether the
//! project tracks any path at or below it (`Written::project_tracks`).
//!
//! Then `ls-files --deleted` names the files absent from disk; a path with an ancestor
//! that exists and is not a directory, by `lstat`, a symbolic link included, is dropped,
//! because Git would write through the link or stop at the file; one the project tracks is
//! left unwritten, because the private version written there would be a change to the
//! project's file that a public `git add` stages (N9); the rest are fed to one
//! `checkout-index` without `--force`, which never overwrites a file and stops at the
//! first path it cannot create, so that it meets none (S7). What is present is looked at
//! only afterwards, by `diff`, whose paths minus the dropped and unwritten ones are the
//! files kept as they were. Every run is made from the root, because `ls-files` lists and
//! `checkout-index` reads paths relative to the directory it runs from. Its runs are the
//! same whatever the repository holds (G22, R4); the only per-path work is `lstat` of the
//! paths Git named and of the region paths' ancestors.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use super::step;
use crate::bytes;
use crate::guards::operand;
use crate::keeper::{self, Failed};
use crate::runner::Run;
use crate::runner::locate::{Workspace, directory_at};
use crate::runner::records;

/// What the write step left as it was, each path relative to the root.
pub struct Written {
    /// The outermost ancestor of each absent file that was not written, once however
    /// many it stood in the way of, in byte order, with what stands there.
    pub obstructions: BTreeMap<Vec<u8>, Standing>,
    /// Each file present before the step that differs from the checked-out version, in
    /// content or in kind, in byte order, with why.
    pub kept: BTreeMap<Vec<u8>, Kept>,
    /// Each absent file that was not written because the project tracks its path, in
    /// byte order.
    pub unwritten: BTreeSet<Vec<u8>>,
    /// The paths the project tracks at or below the listing's query.
    publicly_tracked: BTreeSet<Vec<u8>>,
}

impl Written {
    /// Whether the project tracks `path` or a path below it: where it does, a restore
    /// that names `path` would write private files over the project's, or in place of
    /// what holds them.
    pub fn project_tracks(&self, path: &[u8]) -> bool {
        operand::any_at_or_below(&self.publicly_tracked, path)
    }
}

/// Something at a path that is not a directory, by `lstat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    SymbolicLink,
    /// A file, or anything else that is neither a directory nor a symbolic link.
    File,
}

/// Why a present path differs from the checked-out version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kept {
    /// Its content or its kind.
    Differs,
    /// A directory stands where the checked-out version has a file.
    Directory,
    /// It lies beyond this symbolic link, the outermost among its ancestors.
    BeyondALink(Vec<u8>),
}

/// The write step in the workspace's private repository, once its index is the
/// checked-out branch's tree.
pub fn write(workspace: &Workspace) -> Result<Written, Failed> {
    let private = workspace.private_directory();
    let root = workspace.root();
    let run = |words: &[&str]| Run::private(&private, root, words).from(root);

    let hidden = keeper::hidden(workspace)?;
    let mut known = HashMap::new();
    let publicly_tracked = keeper::publicly_tracked(
        workspace,
        &query(root, &hidden.paths.region, &mut known, |path| {
            hidden.paths.named(path)
        }),
    )?;

    let deleted = step(run(&["ls-files", "-z", "--deleted"]).capture_output(), &[0])?;
    let mut obstructions = BTreeMap::new();
    let mut dropped = BTreeSet::new();
    let mut unwritten = BTreeSet::new();
    let mut absent = Vec::new();
    for path in records::paths(&deleted.stdout) {
        // A path the project tracks is named as such whatever stands above it, so that
        // the line naming an obstruction does not stand in for it.
        let tracked = publicly_tracked.contains(&path);
        match obstruction(root, &path, &mut known) {
            Some((ancestor, standing)) => {
                obstructions.insert(ancestor.to_vec(), standing);
                if tracked {
                    unwritten.insert(path);
                } else {
                    dropped.insert(path);
                }
            }
            None if tracked => {
                unwritten.insert(path);
            }
            None => {
                absent.extend_from_slice(&path);
                absent.push(0);
            }
        }
    }

    // Fed paths git-dupe wrote itself, so no pathspec variable of a global option before
    // `dupe` applies to the run (`Composition/Runner`); it runs with nothing to feed too,
    // so that the runs of `clone` stay the same whatever is on disk.
    let checkout = run(&["checkout-index", "-z", "--stdin", "-u"])
        .own_paths()
        .feed(absent)
        .capture_output();
    step(checkout, &[0])?;

    let differing = step(
        run(&["diff", "--name-only", "-z", "--no-relative"]).capture_output(),
        &[0],
    )?;
    let mut kept = BTreeMap::new();
    let mut known = HashMap::new();
    for path in records::paths(&differing.stdout) {
        if dropped.contains(&path) || unwritten.contains(&path) {
            continue;
        }
        let why = match obstruction(root, &path, &mut known) {
            Some((link, _)) => Kept::BeyondALink(link.to_vec()),
            None if directory_at(root, &path) => Kept::Directory,
            None => Kept::Differs,
        };
        kept.insert(path, why);
    }

    Ok(Written {
        obstructions,
        kept,
        unwritten,
        publicly_tracked,
    })
}

/// The public listing's query: each of the `region` paths that `named` admits, a
/// command line being unable to carry the rest (G30), replaced by its outermost proper
/// ancestor that exists and is not a directory where one stands, each once. A path a
/// line offers to restore is a privately tracked file, or the outermost such ancestor of
/// one, so it lies at or below one of these.
fn query(
    root: &Path,
    region: &[Vec<u8>],
    known: &mut HashMap<Vec<u8>, Option<Standing>>,
    named: impl Fn(&[u8]) -> bool,
) -> Vec<Vec<u8>> {
    let mut query = BTreeSet::new();
    for path in region.iter().filter(|path| named(path)) {
        let outermost = obstruction(root, path, known).map_or(&path[..], |(above, _)| above);
        query.insert(outermost.to_vec());
    }
    query.into_iter().collect()
}

/// The outermost proper ancestor of `path` that exists and is not a directory, by
/// `lstat`, and what stands there. The answers for ancestors already looked at are kept
/// in `known`.
fn obstruction<'p>(
    root: &Path,
    path: &'p [u8],
    known: &mut HashMap<Vec<u8>, Option<Standing>>,
) -> Option<(&'p [u8], Standing)> {
    operand::ancestors(path).find_map(|ancestor| {
        let standing = *known
            .entry(ancestor.to_vec())
            .or_insert_with(|| standing(root, ancestor));
        standing.map(|standing| (ancestor, standing))
    })
}

/// What stands at `path` when it exists and is not a directory, by `lstat`.
fn standing(root: &Path, path: &[u8]) -> Option<Standing> {
    let found = fs::symlink_metadata(bytes::under(root, path)?).ok()?;
    let kind = found.file_type();
    if kind.is_dir() {
        None
    } else if kind.is_symlink() {
        Some(Standing::SymbolicLink)
    } else {
        Some(Standing::File)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::symlink;
    use std::path::PathBuf;

    /// A directory of its own below the system's temporary directory, removed when the
    /// check ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let path =
                std::env::temp_dir().join(format!("git-dupe-write-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_outermost_ancestor_that_is_not_a_directory_obstructs_by_lstat() {
        let scratch = Scratch::new("obstruction");
        let root = &scratch.0;
        fs::create_dir_all(root.join("dir/sub")).unwrap();
        fs::create_dir_all(root.join("target/deep")).unwrap();
        fs::write(root.join("file"), "a file\n").unwrap();
        fs::write(root.join("dir/inner"), "a file below a directory\n").unwrap();
        symlink("target", root.join("link")).unwrap();
        symlink("missing", root.join("dangling")).unwrap();
        symlink("../target", root.join("dir/link")).unwrap();

        let mut known = HashMap::new();
        let mut asked = |path: &'static [u8]| obstruction(root, path, &mut known);
        // Nothing in the way: a path at the root, absent ancestors, directories.
        assert_eq!(asked(b"top"), None);
        assert_eq!(asked(b"absent/x/y"), None);
        assert_eq!(asked(b"dir/sub/x"), None);
        // A file, however deep below it.
        assert_eq!(asked(b"file/x"), Some((&b"file"[..], Standing::File)));
        assert_eq!(asked(b"file/x/y/z"), Some((&b"file"[..], Standing::File)));
        assert_eq!(
            asked(b"dir/inner/x"),
            Some((&b"dir/inner"[..], Standing::File))
        );
        // A symbolic link to a directory, a dangling one, and one below a directory; the
        // outermost is named even where a file stands further in.
        assert_eq!(
            asked(b"link/deep/x"),
            Some((&b"link"[..], Standing::SymbolicLink))
        );
        assert_eq!(
            asked(b"dangling/x"),
            Some((&b"dangling"[..], Standing::SymbolicLink))
        );
        assert_eq!(
            asked(b"dir/link/x"),
            Some((&b"dir/link"[..], Standing::SymbolicLink))
        );
        fs::write(root.join("target/deep/file"), "a file beyond a link\n").unwrap();
        assert_eq!(
            asked(b"link/deep/file/x"),
            Some((&b"link"[..], Standing::SymbolicLink))
        );
        // The path itself is no ancestor of its own.
        assert_eq!(asked(b"file"), None);
        assert_eq!(asked(b"link"), None);
    }

    #[test]
    fn the_query_is_the_region_paths_each_at_its_outermost_obstruction() {
        let scratch = Scratch::new("query");
        let root = &scratch.0;
        fs::create_dir_all(root.join("dir")).unwrap();
        fs::write(root.join("file"), "a file\n").unwrap();
        symlink("dir", root.join("link")).unwrap();
        let region: Vec<Vec<u8>> = [
            &b".gitdupe"[..],
            b"dir/a",
            b"file/x",
            b"file/y/z",
            b"link/b",
            b"notes",
            b"unnamed",
        ]
        .iter()
        .map(|path| path.to_vec())
        .collect();
        let mut known = HashMap::new();
        let query = query(root, &region, &mut known, |path| path != b"unnamed");
        // Never more paths, and never more bytes, than the region paths it was made from.
        let expected: Vec<Vec<u8>> = [&b".gitdupe"[..], b"dir/a", b"file", b"link", b"notes"]
            .iter()
            .map(|path| path.to_vec())
            .collect();
        assert_eq!(query, expected);
    }

    #[test]
    fn an_answer_already_had_is_not_asked_again() {
        let scratch = Scratch::new("known");
        let root = &scratch.0;
        fs::write(root.join("file"), "a file\n").unwrap();
        let mut known = HashMap::new();
        assert!(obstruction(root, b"file/x", &mut known).is_some());
        fs::remove_file(root.join("file")).unwrap();
        // The remembered answer stands for the same ancestor.
        assert!(obstruction(root, b"file/y", &mut known).is_some());
        assert_eq!(known.len(), 1);
    }
}
