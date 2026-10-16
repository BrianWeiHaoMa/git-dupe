//! `clean`'s refusals and patterns, decided over paths and listings written here.

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
fn a_pathspec_is_non_literal_by_its_bytes_whatever_its_magic_says() {
    for word in [
        &b":(exclude)cache/secret"[..],
        b":!cache/secret",
        b":^cache/secret",
        b":(exclude,glob)**/secret",
        b":(attr:-text)cache",
        b":(top)cache",
        b":(icase)CACHE",
        b":(literal)cache",
        b":x",
        b":",
        b"cache?",
        b"*[k/]",
        b"cache*[k/]",
        b"cache*[!t]",
        b":(glob)cache*[k/]",
        b":(icase)CACHE*[K/]",
        b"cache*",
        b"cache\\/",
        b"*e/",
        b"docs/*",
        b"doc?",
        b":/cache*",
        b":/a\\b",
        b":(exclude)a\nb",
    ] {
        assert!(non_literal(word), "{}", word.escape_ascii());
    }
    for word in [
        &b"cache"[..],
        b"cache/",
        b"../cache",
        b".",
        b":/",
        b":/cache",
        b"a b",
    ] {
        assert!(!non_literal(word), "{}", word.escape_ascii());
    }
}

#[test]
fn the_listing_is_asked_under_the_outermost_ancestor_of_each_region_path_once() {
    let region = paths(&[".gitdupe", "cache/secret", "cache/x/y", "docs/sub/notes.md"]);
    assert_eq!(outermost_ancestors(&region), set(&["cache", "docs"]));
    assert!(outermost_ancestors(&paths(&[".gitdupe", "notes"])).is_empty());
}

#[test]
fn a_loose_directory_is_the_outermost_ancestor_the_listing_names_nothing_under() {
    let region = paths(&[".gitdupe", "cache/secret", "deep/a/secret3"]);
    // The outermost, for the first region path in order that has one.
    assert_eq!(
        loose(&region, &set(&["deep/README"])),
        Some((&b"cache/secret"[..], &b"cache"[..]))
    );
    assert_eq!(
        loose(&region, &set(&["cache/README"])),
        Some((&b"deep/a/secret3"[..], &b"deep"[..]))
    );
    // A deeper one, below a directory the project tracks a path in.
    let region = paths(&["docs/sub/notes.md"]);
    assert_eq!(
        loose(&region, &set(&["docs/design.md"])),
        Some((&b"docs/sub/notes.md"[..], &b"docs/sub"[..]))
    );
    // None: every ancestor holds a tracked path, or there is no ancestor.
    let region = paths(&[".gitdupe", "docs/notes.md", "notes", "secret"]);
    assert_eq!(loose(&region, &set(&["docs/design.md"])), None);
    assert_eq!(loose(&paths(&["a/b/x"]), &set(&["a/b/c"])), None);
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
