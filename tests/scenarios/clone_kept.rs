//! What `git dupe clone` finds already present on the second machine (G2, `Holds/G2`, S7,
//! F5, `Product`): every file and directory there is kept as it was, byte for byte, kind,
//! mode, and link target; each that differs from the checked-out version is named, and
//! shows as an unstaged change; nothing is written through a symbolic link or below a file
//! where the private repository has a directory; every other absent file is written, but
//! one outside the sparse set of a sparse checkout a template configures, which stays
//! unwritten as Git leaves it.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, holds, private_commit, region_rules, under_each_release,
    warnings_in_any_order, write, write_executable,
};

/// `path` below `dir`, from its bytes.
fn at(dir: &Path, path: &[u8]) -> PathBuf {
    dir.join(OsStr::from_bytes(path))
}

/// Writes `bytes` at `path` below `dir`, making the directories above it first.
fn file(dir: &Path, path: &[u8], bytes: &[u8]) {
    let path = at(dir, path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Makes `paths`, written below the first machine's root, privately tracked there,
/// committed, and pushed to its private remote.
fn pushed(s: &Scenario, first: &Path, paths: &[&[u8]]) {
    let add = ["add", "-f", "--"].map(OsStr::new);
    let paths = paths.iter().map(|path| OsStr::from_bytes(path));
    s.private(first)
        .git(add.into_iter().chain(paths))
        .succeeds();
    private_commit(s, first);
    s.private(first)
        .git(["push", "-q", "origin", "main"])
        .succeeds();
}

/// `git dupe clone` of the first machine's private remote, from the second machine's root.
fn cloned(s: &Scenario, second: &Path, url: &Path) -> Output {
    let words = [OsStr::new("dupe"), OsStr::new("clone"), url.as_os_str()];
    let output = s.git(words).from(second).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
    for level in ["fatal", "error", "hint"] {
        assert!(output.lines(level).is_empty(), "{level}: {output:?}");
    }
    output
}

/// The entries of `git dupe status --porcelain -z`, each path with its two status letters.
fn status(s: &Scenario, dir: &Path) -> BTreeMap<Vec<u8>, [u8; 2]> {
    let output = s
        .git(["dupe", "status", "--porcelain", "-z"])
        .from(dir)
        .succeeds();
    output
        .stdout
        .split(|&byte| byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| (entry[3..].to_vec(), [entry[0], entry[1]]))
        .collect()
}

/// Every entry of `before` stands as it stood, and the entries added are exactly `added`.
fn kept_and_added(before: &Tree, dir: &Path, added: &[&[u8]]) {
    let mut changed = before.changed_in(&Tree::working(dir));
    changed.sort();
    let mut expected: Vec<PathBuf> = added.iter().map(|path| at(dir, path)).collect();
    expected.sort();
    assert_eq!(changed, expected);
}

#[test]
fn present_files_are_kept_and_named_and_absent_ones_written_as_committed() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let url = &m.first.private_remote;

        // Private history beside `daily_state`'s: an executable, a symbolic link, files
        // whose names are no UTF-8 or hold a space, and the files the second machine has
        // its own of.
        write_executable(s, &first.join("bin/run"), b"#!/bin/sh\n");
        symlink("notes/a.md", first.join("link")).unwrap();
        let committed: [(&[u8], &[u8]); 6] = [
            (b"same.txt", b"same\n"),
            (b"conf", b"the private conf\n"),
            (b"kind", b"a regular file\n"),
            (b"notes/caf\xff", b"absent, its name no UTF-8\n"),
            (b"notes/with space", b"absent, its name with a space\n"),
            (b"notes/caf\xfe", b"private\n"),
        ];
        for (path, bytes) in committed {
            file(first, path, bytes);
        }
        let mut paths: Vec<&[u8]> = committed.iter().map(|(path, _)| *path).collect();
        paths.extend([&b"bin/run"[..], b"link"]);
        pushed(s, first, &paths);

        // The second machine's own: identical, differing in content, a directory and a
        // symbolic link where files are tracked, a name of bytes, and its own `.gitdupe`,
        // which lists `scratch` where the first's lists `.vscode`.
        file(second, b"same.txt", b"same\n");
        file(second, b".env.local", b"mine\n");
        file(second, b"conf/inner", b"the second machine's\n");
        symlink("same.txt", second.join("kind")).unwrap();
        file(second, b"notes/caf\xfe", b"the second machine's\n");
        file(second, b".gitdupe", b"notes\nscratch\n");
        let before = Tree::working(second);

        let output = cloned(s, second, url);
        warnings_in_any_order(
            &output,
            &[
                &[
                    b".env.local was kept",
                    b"differs",
                    b"git dupe restore -- .env.local",
                ],
                &[
                    b".gitdupe was kept",
                    b"differs",
                    b"git dupe restore -- .gitdupe",
                ],
                &[b"conf was kept", b"directory", b"git dupe restore -- conf"],
                &[b"kind was kept", b"differs", b"git dupe restore -- kind"],
                &[b"notes/caf\xfe was kept", b"differs"],
            ],
        );
        // The warnings name each kept path, its cause, and the command that replaces it,
        // never the contents of either version.
        for contents in [
            &b"mine"[..],
            b"the private conf",
            b"the second machine's",
            b"a regular file",
        ] {
            for level in ["fatal", "error", "warning", "hint"] {
                for line in output.lines(level) {
                    assert!(!holds(line, contents), "{output:?}");
                }
            }
        }

        kept_and_added(
            &before,
            second,
            &[
                b"bin",
                b"bin/run",
                b"link",
                b"notes/a.md",
                b"notes/caf\xff",
                b"notes/with space",
                b".vscode",
                b".vscode/settings.json",
                b"docs/notes.md",
            ],
        );
        let run = fs::symlink_metadata(second.join("bin/run")).unwrap();
        assert!(run.is_file() && run.permissions().mode() & 0o111 != 0);
        assert_eq!(fs::read(second.join("bin/run")).unwrap(), b"#!/bin/sh\n");
        assert_eq!(
            fs::read_link(second.join("link")).unwrap(),
            Path::new("notes/a.md")
        );
        for (path, bytes) in &committed[3..5] {
            assert_eq!(fs::read(at(second, path)).unwrap(), *bytes);
        }

        // Each kept file that differs shows as an unstaged change; the identical one and
        // every file written show nothing.
        let changes = status(s, second);
        for path in [
            &b".env.local"[..],
            b".gitdupe",
            b"conf",
            b"kind",
            b"notes/caf\xfe",
        ] {
            let found = changes.get(path).unwrap_or_else(|| panic!("{changes:?}"));
            assert_ne!(found[1], b' ', "{}: {changes:?}", path.escape_ascii());
        }
        for path in [
            &b"same.txt"[..],
            b"bin/run",
            b"link",
            b"notes/caf\xff",
            b"notes/with space",
        ] {
            assert!(!changes.contains_key(path), "{changes:?}");
        }

        // The hidden paths follow the kept `.gitdupe`: `scratch` has a rule, `.vscode` none,
        // its privately tracked file still one.
        let rules = region_rules(second);
        assert!(rules.iter().any(|rule| rule == b"/scratch"), "{rules:?}");
        assert!(!rules.iter().any(|rule| rule == b"/.vscode"), "{rules:?}");
        assert!(
            rules.iter().any(|rule| rule == b"/.vscode/settings.json"),
            "{rules:?}"
        );
    });
}

#[test]
fn an_obstruction_is_named_once_and_nothing_is_written_through_or_below_it() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let url = &m.first.private_remote;

        let committed: [&[u8]; 9] = [
            b"aa/first",
            b"tree/one",
            b"tree/two",
            b"zz/last",
            b"lnk/a",
            b"lnk/b",
            b"dangling/x",
            b"deep/inner/file",
            b"deep/other",
        ];
        for path in committed {
            file(first, path, b"private\n");
        }
        pushed(s, first, &committed);

        // A file where a directory is tracked, with absent files sorting before and after
        // it; a symbolic link to a directory holding one of its tracked files and lacking
        // the other; a dangling one; and a file below a directory that exists.
        file(
            second,
            b"tree",
            b"a file where the private repository has a directory\n",
        );
        write(second, "target/a", b"private\n");
        symlink("target", second.join("lnk")).unwrap();
        symlink("nowhere", second.join("dangling")).unwrap();
        file(second, b"deep/inner", b"a file below a directory\n");
        let before = Tree::working(second);

        let output = cloned(s, second, url);
        warnings_in_any_order(
            &output,
            &[
                &[b"tree was kept", b"a file", b"git dupe restore -- tree"],
                &[
                    b"lnk was kept",
                    b"symbolic link",
                    b"git dupe restore -- lnk",
                ],
                &[b"dangling was kept", b"symbolic link"],
                &[
                    b"deep/inner was kept",
                    b"a file",
                    b"git dupe restore -- deep/inner",
                ],
                // Present through the link: kept, and named for it.
                &[
                    b"lnk/a was kept",
                    b"symbolic link lnk",
                    b"git dupe restore -- lnk",
                ],
                // Settle's own: hidden paths beyond a link, which public Git never sees.
                &[b"lnk/a lies beyond a symbolic link"],
                &[b"lnk/b lies beyond a symbolic link"],
                &[b"dangling/x lies beyond a symbolic link"],
            ],
        );

        // Nothing that was there changed, the link targets and what they hold included;
        // the files no obstruction stands above are written.
        kept_and_added(
            &before,
            second,
            &[
                b"aa",
                b"aa/first",
                b"zz",
                b"zz/last",
                b"deep/other",
                b".env.local",
                b".gitdupe",
                b"notes",
                b"notes/a.md",
                b".vscode",
                b".vscode/settings.json",
                b"docs/notes.md",
            ],
        );
        assert!(!second.join("target/b").exists());
        assert!(!second.join("nowhere").exists());

        // The files below an obstruction are unwritten and show as deleted.
        let changes = status(s, second);
        for path in [
            &b"tree/one"[..],
            b"tree/two",
            b"lnk/b",
            b"dangling/x",
            b"deep/inner/file",
        ] {
            let found = changes.get(path).unwrap_or_else(|| panic!("{changes:?}"));
            assert_eq!(found[1], b'D', "{}: {changes:?}", path.escape_ascii());
        }
        for path in [&b"aa/first"[..], b"zz/last", b"deep/other"] {
            assert!(!changes.contains_key(path), "{changes:?}");
        }
    });
}

/// A template's sparse checkout is retained (G1): clone keeps present entries and
/// writes absent files only inside the sparse set (G2, `Holds/G2`, S7).
#[test]
fn clone_keeps_present_entries_and_leaves_sparse_exclusions_unwritten() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        file(first, b"notes/absent.md", b"inside the sparse set\n");
        pushed(s, first, &[b"notes/absent.md"]);
        file(second, b"notes/a.md", b"the second machine's notes\n");
        file(second, b"local/keep", b"already present\n");
        symlink("local/keep", second.join("local-link")).unwrap();
        let before = Tree::working(second);

        let template = s.dir().join("sparse-template");
        write(
            &template,
            "config",
            b"[core]\n\tsparseCheckout = true\n\tsparseCheckoutCone = false\n",
        );
        let patterns = b"/.gitdupe\n/notes/\n";
        write(&template, "info/sparse-checkout", patterns);
        assert!(!template.join("HEAD").exists());
        let setting = format!("init.templateDir={}", template.display());
        let output = s
            .git([
                OsStr::new("-c"),
                OsStr::new(&setting),
                OsStr::new("dupe"),
                OsStr::new("clone"),
                m.first.private_remote.as_os_str(),
            ])
            .from(second)
            .succeeds();
        warnings_in_any_order(
            &output,
            &[&[
                b"notes/a.md was kept",
                b"differs",
                b"git dupe restore -- notes/a.md",
            ]],
        );
        for level in ["fatal", "error", "hint"] {
            assert!(output.lines(level).is_empty(), "{output:?}");
        }
        kept_and_added(&before, second, &[b".gitdupe", b"notes/absent.md"]);
        for path in [".gitdupe", "notes/absent.md"] {
            assert_eq!(
                fs::read(second.join(path)).unwrap(),
                fs::read(first.join(path)).unwrap(),
                "{path}"
            );
        }
        assert_eq!(
            fs::read(second.join(".git/dupe/info/sparse-checkout")).unwrap(),
            patterns
        );
        let private = s.private(second);
        let sparse = private
            .git(["config", "--bool", "core.sparseCheckout"])
            .succeeds();
        assert_eq!(sparse.stdout, b"true\n");
        let index = private.git(["ls-files", "-t", "-z"]).succeeds();
        let deleted = private.git(["ls-files", "--deleted", "-z"]).succeeds();
        let diff = private.git(["diff", "--name-only", "-z"]).succeeds();
        assert_eq!(diff.stdout, b"notes/a.md\0");
        assert!(deleted.stdout.is_empty(), "{deleted:?}");
        for path in [".env.local", ".vscode/settings.json", "docs/notes.md"] {
            assert_eq!(
                fs::symlink_metadata(second.join(path)).unwrap_err().kind(),
                std::io::ErrorKind::NotFound,
                "{path}"
            );
            let skipped = format!("S {path}");
            assert!(
                index
                    .stdout
                    .split(|&b| b == 0)
                    .any(|entry| entry == skipped.as_bytes()),
                "{index:?}"
            );
            for answer in [&deleted, &diff] {
                assert!(
                    !answer
                        .stdout
                        .split(|&b| b == 0)
                        .any(|entry| entry == path.as_bytes()),
                    "{answer:?}"
                );
            }
            for level in ["fatal", "error", "warning", "hint"] {
                for line in output.lines(level) {
                    assert!(
                        !line.windows(path.len()).any(|part| part == path.as_bytes()),
                        "{output:?}"
                    );
                }
            }
        }
    });
}
