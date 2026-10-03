//! The write step of `clone` (G2, `Holds/G2`): every privately tracked file absent from
//! the working tree is written, and nothing that is there is touched.
//!
//! The order is the guarantee. `ls-files --deleted` names the files absent from disk; a
//! path with an ancestor that exists and is not a directory, by `lstat`, a symbolic link
//! included, is dropped, because Git would write through the link or stop at the file;
//! the rest are fed to one `checkout-index` without `--force`, which never overwrites a
//! file and stops at the first path it cannot create, so that it meets none (S7). What is
//! present is looked at only afterwards, by `diff`, whose paths minus the dropped ones are
//! the files kept as they were. Every run is private and made from the root, because
//! `ls-files` lists and `checkout-index` reads paths relative to the directory it runs
//! from. Three runs, whatever the repository holds (G22, R4); the only per-path work is
//! `lstat` of the paths Git named.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::step;
use crate::guards::operand;
use crate::keeper::Failed;
use crate::runner::Run;
use crate::runner::records;

/// What the write step left as it was, each path relative to the root.
pub struct Written {
    /// The outermost ancestor of each absent file that was not written, once however
    /// many it stood in the way of, in byte order, with what stands there.
    pub obstructions: BTreeMap<Vec<u8>, Standing>,
    /// Each file present before the step that differs from the checked-out version, in
    /// content or in kind, in byte order, with why.
    pub kept: BTreeMap<Vec<u8>, Kept>,
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
    /// It lies beyond this symbolic link, the outermost among its ancestors, where Git
    /// does not look.
    BeyondALink(Vec<u8>),
}

/// The write step in the private repository at `private`, whose working tree is `root`,
/// once its index is the checked-out branch's tree.
pub fn write(private: &Path, root: &Path) -> Result<Written, Failed> {
    let run = |words: &[&str]| Run::private(private, root, words).from(root);

    let deleted = step(run(&["ls-files", "-z", "--deleted"]).capture_output(), &[0])?;
    let mut obstructions = BTreeMap::new();
    let mut dropped = BTreeSet::new();
    let mut absent = Vec::new();
    let mut known = HashMap::new();
    for path in records::paths(&deleted.stdout) {
        match obstruction(root, &path, &mut known) {
            Some((ancestor, standing)) => {
                obstructions.insert(ancestor.to_vec(), standing);
                dropped.insert(path);
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
        if dropped.contains(&path) {
            continue;
        }
        let why = match obstruction(root, &path, &mut known) {
            Some((link, _)) => Kept::BeyondALink(link.to_vec()),
            None if is_directory(root, &path) => Kept::Directory,
            None => Kept::Differs,
        };
        kept.insert(path, why);
    }

    Ok(Written { obstructions, kept })
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
    let found = fs::symlink_metadata(root.join(OsStr::from_bytes(path))).ok()?;
    let kind = found.file_type();
    if kind.is_dir() {
        None
    } else if kind.is_symlink() {
        Some(Standing::SymbolicLink)
    } else {
        Some(Standing::File)
    }
}

fn is_directory(root: &Path, path: &[u8]) -> bool {
    fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_ok_and(|found| found.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
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
