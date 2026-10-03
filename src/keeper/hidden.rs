//! The hidden paths and the region paths.
//!
//! The hidden paths are `.gitdupe`, every path it lists, and every privately tracked
//! file; they are declared, never derived from the working tree. The region paths are
//! the hidden paths not below another hidden path, in byte order: one rule each is all
//! the region holds (F3).

use std::collections::BTreeSet;

use super::gitdupe;
use crate::guards::operand;

pub struct HiddenPaths {
    /// Every hidden path, in byte order.
    pub hidden: BTreeSet<Vec<u8>>,
    /// The hidden paths not below another, in byte order.
    pub region: Vec<Vec<u8>>,
    pub privately_tracked: BTreeSet<Vec<u8>>,
}

impl HiddenPaths {
    /// The hidden paths of the paths `.gitdupe` lists and the privately tracked files.
    pub fn of(listed: Vec<Vec<u8>>, privately_tracked: BTreeSet<Vec<u8>>) -> HiddenPaths {
        let mut hidden: BTreeSet<Vec<u8>> = listed.into_iter().collect();
        hidden.insert(gitdupe::NAME.to_vec());
        hidden.extend(privately_tracked.iter().cloned());
        let region = hidden
            .iter()
            .filter(|path| !operand::ancestors(path).any(|above| hidden.contains(above)))
            .cloned()
            .collect();
        HiddenPaths {
            hidden,
            region,
            privately_tracked,
        }
    }

    /// Whether nothing is hidden but `.gitdupe`, which always is.
    pub fn only_gitdupe(&self) -> bool {
        self.hidden.len() == 1
    }

    /// Whether `path` is hidden: a hidden path, or below one.
    pub fn hides(&self, path: &[u8]) -> bool {
        self.hidden.contains(path)
            || operand::ancestors(path).any(|above| self.hidden.contains(above))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hidden(listed: &[&[u8]], tracked: &[&[u8]]) -> HiddenPaths {
        HiddenPaths::of(
            listed.iter().map(|path| path.to_vec()).collect(),
            tracked.iter().map(|path| path.to_vec()).collect(),
        )
    }

    #[test]
    fn a_path_below_another_hidden_path_has_no_rule_of_its_own() {
        let found = hidden(
            &[b"notes", b"docs/plan.md", b"notes/sub", b"a/b", b"notes"],
            &[b"notes/today.md", b"conf/local.ini", b".gitdupe"],
        );
        assert_eq!(
            found.region,
            [
                &b".gitdupe"[..],
                b"a/b",
                b"conf/local.ini",
                b"docs/plan.md",
                b"notes"
            ]
        );
        assert!(found.hidden.contains(&b"notes/sub"[..]));
        assert!(found.hidden.contains(&b"notes/today.md"[..]));
    }

    #[test]
    fn gitdupe_is_always_hidden() {
        let found = hidden(&[], &[]);
        assert_eq!(found.region, [b".gitdupe"]);
        assert!(found.hides(b".gitdupe"));
    }

    #[test]
    fn below_means_below_a_whole_component() {
        let found = hidden(&[b"notes", b"notes-old", b"notes.d/x"], &[]);
        assert_eq!(
            found.region,
            [&b".gitdupe"[..], b"notes", b"notes-old", b"notes.d/x"]
        );
        assert!(found.hides(b"notes/a/b"));
        assert!(!found.hides(b"notesx"));
        assert!(!found.hides(b"notes.d"));
    }

    #[test]
    fn paths_are_in_byte_order() {
        let found = hidden(&[b"b", b"a/x", b"a-x", b"\xe9", b"B"], &[]);
        assert_eq!(
            found.region,
            [&b".gitdupe"[..], b"B", b"a-x", b"a/x", b"b", b"\xe9"]
        );
    }
}
