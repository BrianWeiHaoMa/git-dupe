//! `clean`'s refusals and the `-e` patterns that make Git's own `clean` spare the hidden
//! paths (G16, `Holds/G16`), decided over what the caller found: the symbolic links
//! `lstat` found among the hidden paths' ancestors, the region paths, the
//! ancestor directories `lstat` found holding a `.git` entry and, under `-X` without `-d`
//! or a pathspec, the region paths it found to be directories, the tracked paths at or
//! below either in the index `clean` reads, and, under `-X`, the ancestors public Git
//! ignores. This part runs nothing and reads no file; the rule of a path is the keeper's
//! one rule form, handed in by the caller, so that nothing here escapes a path a second
//! way.
//!
//! What the code below cannot show:
//!
//! - The patterns are ignore rules of the highest precedence, read in order, the later
//!   deciding (S4). So the user's own `-e` patterns stand before these, and among these
//!   an ancestor's pair stands before every pair below it and before every spared path's.
//! - Git adds no pathspec of git-dupe's to the run: a pathspec makes `clean` delete
//!   untracked directories without `-d` (S4). Everything is said through `-e`.

use std::collections::BTreeSet;

use super::operand;

/// Whether the words are refused before any run of `clean`'s own: `-X`, whose candidates
/// are the ignored paths that git-dupe's patterns un-ignore, given together with a pattern
/// of the user's own, in any spelling (G16). Which words spell either is the front's
/// reading; a hidden path beyond a symbolic link refuses first (`refused_beyond_a_link`).
pub fn refused(only_ignored: bool, users_patterns: bool) -> bool {
    only_ignored && users_patterns
}

/// The refusal of every form while a hidden path has a symbolic link among its
/// ancestors (G16): the first of `hidden`, in their order, for which `link_above`, the
/// caller's `lstat`, finds one, with the outermost such link. Git never looks beyond the
/// link, and its `clean` deletes the link, and what it leads to, as one entry (S5), which
/// no pattern of a hidden path can spare.
pub fn refused_beyond_a_link<'p>(
    hidden: impl IntoIterator<Item = &'p [u8]>,
    mut link_above: impl FnMut(&'p [u8]) -> Option<&'p [u8]>,
) -> Option<(&'p [u8], &'p [u8])> {
    hidden
        .into_iter()
        .find_map(|path| link_above(path).map(|link| (path, link)))
}

/// What the user's words ask of the patterns: Git's standard rules or `-x`, where a
/// spared path is excluded, or `-X`, where it is un-ignored.
pub enum Arrangement<'a> {
    /// Without `-X`: each spared path excluded, so neither it nor anything below it is a
    /// candidate.
    Excluded,
    /// With `-X`: the ancestors public Git ignores lifted for what lies directly below
    /// them, each spared path un-ignored, and, when `keep_directories_whole`, a directory
    /// at a spared path kept out as one ignored entry, unless `publicly_tracked` holds a
    /// path at or below it.
    Unignored {
        ignored_ancestors: &'a BTreeSet<Vec<u8>>,
        /// The words hold neither `-d` nor a pathspec.
        keep_directories_whole: bool,
        /// The paths the caller's listing found in the index the `clean` run reads. Git
        /// enters a directory under which that index holds a path whatever the patterns
        /// say, and everything below an excluded directory is ignored (S4), so such a
        /// directory at a spared path is never kept out: what lies in it would be taken.
        publicly_tracked: &'a BTreeSet<Vec<u8>>,
    },
}

/// The nested repositories as G16 counts them: each directory of `holding_git`, the
/// ancestors of the region paths found holding a `.git` entry, whether or not Git would
/// open it, at or below which `publicly_tracked`, the paths under them in the index the
/// `clean` run reads, holds none. Git enters a directory under which that index tracks a
/// path like any other, whatever stands in it (S4), so the region paths in it are spared
/// one by one.
pub fn nested(
    holding_git: &BTreeSet<Vec<u8>>,
    publicly_tracked: &BTreeSet<Vec<u8>>,
) -> BTreeSet<Vec<u8>> {
    holding_git
        .iter()
        .filter(|directory| !operand::any_at_or_below(publicly_tracked, directory))
        .cloned()
        .collect()
}

/// The spared paths: each region path that no directory in `nested` lies above, and, in
/// place of the others, the outermost directory in `nested` above each, once. `nested`
/// holds the nested repositories as G16 counts them (`nested`). In the order of
/// `region`, the first place each is reached.
pub fn spared(region: &[Vec<u8>], nested: &BTreeSet<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut spared: Vec<Vec<u8>> = Vec::new();
    for path in region {
        let outermost = operand::ancestors(path).find(|ancestor| nested.contains(*ancestor));
        let path = outermost.unwrap_or(path);
        if !spared.iter().any(|kept| kept == path) {
            spared.push(path.to_vec());
        }
    }
    spared
}

/// Every proper ancestor below the root of each spared path, each once: what the `-X`
/// question asks public Git about, once the caller has left out each at which something
/// other than a directory stands, a symbolic link included, and each beyond a link.
pub fn ancestors(spared: &[Vec<u8>]) -> BTreeSet<Vec<u8>> {
    spared
        .iter()
        .flat_map(|path| operand::ancestors(path))
        .map(<[u8]>::to_vec)
        .collect()
}

/// The `-e` options for `spared`, as the words `-e` and a pattern, in order, `rule` giving
/// the region rule of a path (`Composition/Keeper`).
pub fn patterns(
    spared: &[Vec<u8>],
    arrangement: &Arrangement,
    rule: impl Fn(&[u8]) -> Vec<u8>,
) -> Vec<Vec<u8>> {
    let mut patterns = Vec::new();
    match arrangement {
        Arrangement::Excluded => {
            for path in spared {
                patterns.push(rule(path));
            }
        }
        Arrangement::Unignored {
            ignored_ancestors,
            keep_directories_whole,
            publicly_tracked,
        } => {
            // Byte order puts each ancestor before every path below it: outermost first.
            for ancestor in ignored_ancestors.iter() {
                let rule = rule(ancestor);
                patterns.push([b"!", &rule[..]].concat());
                patterns.push([&rule[..], b"/*"].concat());
            }
            for path in spared {
                let rule = rule(path);
                patterns.push([b"!", &rule[..]].concat());
                patterns.push([b"!", &rule[..], b"/**"].concat());
                if *keep_directories_whole && !operand::any_at_or_below(publicly_tracked, path) {
                    patterns.push([&rule[..], b"/"].concat());
                }
            }
        }
    }
    patterns
        .into_iter()
        .flat_map(|pattern| [b"-e".to_vec(), pattern])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the region rule: `/` before the path, `\` before a space. The rule
    /// form itself is the keeper's and checked there.
    fn rule(path: &[u8]) -> Vec<u8> {
        let mut rule = b"/".to_vec();
        for &byte in path {
            if byte == b' ' {
                rule.push(b'\\');
            }
            rule.push(byte);
        }
        rule
    }

    fn paths(paths: &[&str]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.as_bytes().to_vec()).collect()
    }

    fn set(paths: &[&str]) -> BTreeSet<Vec<u8>> {
        paths.iter().map(|path| path.as_bytes().to_vec()).collect()
    }

    /// The options as the strings they are, `-e` words included.
    fn written(options: Vec<Vec<u8>>) -> Vec<String> {
        options
            .into_iter()
            .map(|word| String::from_utf8(word).unwrap())
            .collect()
    }

    #[test]
    fn only_x_beside_a_pattern_of_the_users_is_refused() {
        assert!(refused(true, true));
        assert!(!refused(true, false));
        assert!(!refused(false, true));
        assert!(!refused(false, false));
    }

    #[test]
    fn the_first_hidden_path_beyond_a_link_is_refused_with_the_link_found() {
        let hidden = paths(&[".gitdupe", "a/x", "b/link/y", "c/link/z"]);
        // A stand-in for `lstat`: `b/link` and `c/link` are links.
        let found = refused_beyond_a_link(hidden.iter().map(Vec::as_slice), |path| {
            (path.starts_with(b"b/") || path.starts_with(b"c/")).then(|| &path[..6])
        });
        assert_eq!(found, Some((&b"b/link/y"[..], &b"b/link"[..])));
        let none = refused_beyond_a_link(hidden.iter().map(Vec::as_slice), |_| None);
        assert_eq!(none, None);
    }

    #[test]
    fn a_directory_holding_a_git_entry_is_nested_only_while_the_project_tracks_nothing_in_it() {
        let holding_git = set(&["opened", "outer", "outer/inner", "tracked", "gitlink"]);
        // `outer` holds a tracked path below its inner repository, which is no more
        // nested than it; `gitlink` is itself an entry of the public index.
        let publicly_tracked = set(&["README.md", "outer/inner/x", "tracked/a/b", "gitlink"]);
        assert_eq!(nested(&holding_git, &publicly_tracked), set(&["opened"]));
        // A tracked path beside a directory, or named like it, is not under it.
        let publicly_tracked = set(&["opened.txt", "opene", "outer2/x"]);
        assert_eq!(nested(&holding_git, &publicly_tracked), holding_git);
        assert!(nested(&set(&[]), &publicly_tracked).is_empty());
        // A region path in a tracked directory is spared alone; an untracked repository
        // inside such a directory is still spared whole.
        let region = paths(&["tracked/secret", "tracked/inner/x", "tracked/inner/y"]);
        let holding_git = set(&["tracked", "tracked/inner"]);
        let nested = nested(&holding_git, &set(&["tracked/public"]));
        assert_eq!(nested, set(&["tracked/inner"]));
        assert_eq!(
            spared(&region, &nested),
            paths(&["tracked/secret", "tracked/inner"])
        );
    }

    #[test]
    fn a_nested_repository_above_a_region_path_is_spared_in_its_place() {
        let region = paths(&[".gitdupe", "notes", "vendor/lib/secret.txt"]);
        assert_eq!(
            spared(&region, &set(&["vendor/lib"])),
            paths(&[".gitdupe", "notes", "vendor/lib"])
        );
        // No nested repository: every region path, in order.
        assert_eq!(spared(&region, &set(&[])), region);
        // A directory found holding `.git` that lies above no region path changes nothing.
        assert_eq!(spared(&region, &set(&["other"])), region);
    }

    #[test]
    fn one_nested_repository_above_two_region_paths_is_spared_once() {
        let region = paths(&["a.txt", "vendor/lib/one", "vendor/lib/two/x", "z"]);
        assert_eq!(
            spared(&region, &set(&["vendor/lib"])),
            paths(&["a.txt", "vendor/lib", "z"])
        );
    }

    #[test]
    fn of_two_nested_repositories_one_inside_the_other_the_outermost_is_spared() {
        let region = paths(&["outer/inner/secret", "outer/beside"]);
        let nested = set(&["outer", "outer/inner"]);
        assert_eq!(spared(&region, &nested), paths(&["outer"]));
        // The inner one alone.
        let nested = set(&["outer/inner"]);
        assert_eq!(
            spared(&region, &nested),
            paths(&["outer/inner", "outer/beside"])
        );
    }

    #[test]
    fn the_ancestors_asked_about_are_every_proper_ancestor_once() {
        let spared = paths(&["a/b/c", "a/d", "top", "a/b/e/f"]);
        assert_eq!(ancestors(&spared), set(&["a", "a/b", "a/b/e"]));
        assert!(ancestors(&paths(&["top"])).is_empty());
    }

    #[test]
    fn without_x_each_spared_path_is_one_exclusion() {
        let spared = paths(&["notes", "has space/x"]);
        assert_eq!(
            written(patterns(&spared, &Arrangement::Excluded, rule)),
            ["-e", "/notes", "-e", "/has\\ space/x"]
        );
        assert!(patterns(&[], &Arrangement::Excluded, rule).is_empty());
    }

    #[test]
    fn with_x_ignored_ancestors_come_outermost_first_then_each_spared_path() {
        let spared = paths(&["build/deep/keep.txt", "notes", "build/local.cfg"]);
        let ignored = set(&["build/deep", "build"]);
        let tracked = set(&[]);
        let arrangement = Arrangement::Unignored {
            ignored_ancestors: &ignored,
            keep_directories_whole: true,
            publicly_tracked: &tracked,
        };
        assert_eq!(
            written(patterns(&spared, &arrangement, rule)),
            [
                "-e",
                "!/build",
                "-e",
                "/build/*",
                "-e",
                "!/build/deep",
                "-e",
                "/build/deep/*",
                "-e",
                "!/build/deep/keep.txt",
                "-e",
                "!/build/deep/keep.txt/**",
                "-e",
                "/build/deep/keep.txt/",
                "-e",
                "!/notes",
                "-e",
                "!/notes/**",
                "-e",
                "/notes/",
                "-e",
                "!/build/local.cfg",
                "-e",
                "!/build/local.cfg/**",
                "-e",
                "/build/local.cfg/",
            ]
        );
    }

    #[test]
    fn with_x_a_spared_path_the_index_holds_a_path_at_or_below_is_not_kept_out_whole() {
        let spared = paths(&["notes", "ign/keep", "pile/h", "vendor/lib"]);
        let ignored = set(&["ign"]);
        // A tracked path beside a spared path, or named like it, is not under it; a
        // gitlink at a spared path is at it.
        let tracked = set(&[
            "notes/shared.md",
            "ign/keep/a/b",
            "pile/h.txt",
            "vendor/lib",
        ]);
        let arrangement = Arrangement::Unignored {
            ignored_ancestors: &ignored,
            keep_directories_whole: true,
            publicly_tracked: &tracked,
        };
        assert_eq!(
            written(patterns(&spared, &arrangement, rule)),
            [
                "-e",
                "!/ign",
                "-e",
                "/ign/*",
                "-e",
                "!/notes",
                "-e",
                "!/notes/**",
                "-e",
                "!/ign/keep",
                "-e",
                "!/ign/keep/**",
                "-e",
                "!/pile/h",
                "-e",
                "!/pile/h/**",
                "-e",
                "/pile/h/",
                "-e",
                "!/vendor/lib",
                "-e",
                "!/vendor/lib/**",
            ]
        );
    }

    #[test]
    fn with_x_and_d_or_a_pathspec_no_directory_is_kept_out_whole() {
        let spared = paths(&["notes"]);
        let ignored = set(&[]);
        let tracked = set(&[]);
        let arrangement = Arrangement::Unignored {
            ignored_ancestors: &ignored,
            keep_directories_whole: false,
            publicly_tracked: &tracked,
        };
        assert_eq!(
            written(patterns(&spared, &arrangement, rule)),
            ["-e", "!/notes", "-e", "!/notes/**"]
        );
        let ignored = set(&["ign"]);
        let arrangement = Arrangement::Unignored {
            ignored_ancestors: &ignored,
            keep_directories_whole: false,
            publicly_tracked: &tracked,
        };
        assert_eq!(
            written(patterns(&paths(&["ign/keep"]), &arrangement, rule)),
            [
                "-e",
                "!/ign",
                "-e",
                "/ign/*",
                "-e",
                "!/ign/keep",
                "-e",
                "!/ign/keep/**"
            ]
        );
    }
}
