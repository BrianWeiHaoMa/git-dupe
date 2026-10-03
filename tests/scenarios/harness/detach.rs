//! What `detach` says and leaves, for the scenarios of a whole `detach` and of a killed
//! one: the warning naming a formerly hidden path, those of the daily state, a run's
//! warnings exactly and the closing hint that counts them, and a workspace left
//! unattached. Before a refusal of `detach` is compared byte for byte, a privately tracked
//! file's timestamp is made stale, so that the status run `detach` reads would rewrite the
//! private index if it could (`Holds/G3`).

use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use super::attached::region;
use super::output::Output;

/// The hidden paths of `daily_state` that public Git can see once `detach` has left:
/// `.env.local` and `.vscode` stay ignored by the project's `.gitignore`.
const DAILY_EXPOSED: &[&str] = &[".gitdupe", "notes", "notes/a.md", "docs/notes.md"];

/// The text of `detach`'s warning naming `path`, formerly hidden, as now visible.
pub fn now_visible(path: &str) -> Vec<u8> {
    format!("{path} was hidden and is now visible to public Git").into_bytes()
}

/// The texts of `detach`'s warnings in `daily_state`.
pub fn daily_warnings() -> Vec<Vec<u8>> {
    DAILY_EXPOSED.iter().map(|path| now_visible(path)).collect()
}

/// The text of `detach`'s closing hint, after warnings naming `count` formerly hidden
/// paths as visible to public Git.
fn closing(count: usize) -> Vec<u8> {
    let (paths, are, them) = match count {
        1 => ("path", "is", "it"),
        _ => ("paths", "are", "them"),
    };
    format!(
        "{count} formerly hidden {paths} named above {are} now visible to public Git; \
         check 'git status' before 'git add -A' stages {them}"
    )
    .into_bytes()
}

/// Exact warning lines, then, where they name any path as visible to public Git, one
/// closing `hint:` counting those, with no Git errors or other stderr mixed in. A warning
/// that names no path as visible, as one beyond a symbolic link does, is not counted.
pub fn warnings(output: &Output, expected: Vec<Vec<u8>>) {
    let visible = expected
        .iter()
        .filter(|warning| warning.ends_with(b"visible to public Git"))
        .count();
    let mut expected = expected;
    expected.sort();
    let mut found: Vec<Vec<u8>> = output
        .lines("warning")
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect();
    found.sort();
    assert_eq!(found, expected, "{output:?}");
    let hints = output.lines("hint");
    if visible == 0 {
        assert!(hints.is_empty(), "{output:?}");
    } else {
        assert_eq!(hints, [closing(visible)], "{output:?}");
        let last = [&b"hint: "[..], &closing(visible), b"\n"].concat();
        assert!(output.stderr.ends_with(&last), "{output:?}");
    }
    assert_eq!(
        output.stderr.split_inclusive(|&b| b == b'\n').count(),
        found.len() + hints.len(),
        "{output:?}"
    );
}

/// The workspace at `root` is unattached: no private repository and no region.
pub fn detached(root: &Path) {
    assert!(!root.join(".git/dupe").exists());
    assert!(region(root).is_none());
}

/// A status without `--no-optional-locks` would refresh the private index here.
/// Do this after fixture Git commands, which can refresh the index themselves.
pub fn stale_timestamp(root: &Path) {
    stale_file_timestamp(&root.join("docs/notes.md"));
}

pub fn stale_file_timestamp(file: &Path) {
    fs::File::options()
        .write(true)
        .open(file)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000_000))
        .unwrap();
}
