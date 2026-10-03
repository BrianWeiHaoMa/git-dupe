//! The hidden paths under a path: the one rule by which `status` and `add` confine Git to
//! what lies at or below a path the user named (G12, G13).

use std::collections::BTreeSet;

use super::operand;

/// The hidden paths at or below `path`, root-relative, given the region paths: the region
/// paths at or below it, or `path` itself when it lies below a region path; for the root,
/// the empty path, every region path. The two cases exclude each other, because no region
/// path lies below another.
pub fn hidden_under(region: &[Vec<u8>], path: &[u8]) -> Vec<Vec<u8>> {
    // The ancestor relation answers nothing for the root, which is this rule's own case.
    if path.is_empty() {
        return region.to_vec();
    }
    let regions: BTreeSet<&[u8]> = region.iter().map(Vec::as_slice).collect();
    if operand::ancestors(path).any(|above| regions.contains(above)) {
        return vec![path.to_vec()];
    }
    region
        .iter()
        .filter(|hidden| operand::at_or_below(hidden, path))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    #[test]
    fn the_region_paths_at_or_below_a_path_or_the_path_below_a_region_path() {
        let hidden = region(&[b".gitdupe", b".vscode", b"docs/notes.md", b"notes"]);
        assert_eq!(hidden_under(&hidden, b"notes"), region(&[b"notes"]));
        assert_eq!(hidden_under(&hidden, b"docs"), region(&[b"docs/notes.md"]));
        assert_eq!(hidden_under(&hidden, b"notes/sub"), region(&[b"notes/sub"]));
        assert_eq!(
            hidden_under(&hidden, b"notes/sub/x"),
            region(&[b"notes/sub/x"])
        );
        assert_eq!(
            hidden_under(&hidden, b"docs/notes.md"),
            region(&[b"docs/notes.md"])
        );
        // Whole components: `notes-old` lies beside `notes`, not below it.
        assert!(hidden_under(&hidden, b"notes-old").is_empty());
        assert!(hidden_under(&hidden, b"src").is_empty());
        assert!(hidden_under(&hidden, b"doc").is_empty());
    }

    #[test]
    fn the_root_is_every_region_path() {
        let hidden = region(&[b".gitdupe", b"notes"]);
        assert_eq!(hidden_under(&hidden, b""), hidden);
        assert_eq!(
            hidden_under(&region(&[b".gitdupe"]), b""),
            region(&[b".gitdupe"])
        );
    }
}
