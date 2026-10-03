//! `clone` runs a fixed Git sequence at either private file count (G22, R4), and a
//! caller's index reaches neither private runs nor public tracking queries (G10,
//! `Composition/Runner`).

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Scenario, Tree, names, private_add, private_commit, region_rules, run_traced,
    under_each_release, write,
};

/// The private index's entries, including modes, object names, and stages.
fn private_index(s: &Scenario, root: &Path) -> Vec<u8> {
    s.private(root)
        .git(["ls-files", "-s", "-z"])
        .succeeds()
        .stdout
}

/// The private commit at the attached machine's `HEAD`.
fn private_head(s: &Scenario, root: &Path) -> Vec<u8> {
    s.private(root).git(["rev-parse", "HEAD"]).succeeds().stdout
}

#[test]
fn clone_git_run_sequences_do_not_follow_private_file_counts() {
    under_each_release(|s| {
        for branch in [false, true] {
            let mut sequences = Vec::new();
            for files in [5, 500] {
                let m = s.second_machine(&format!("runs-{branch}-{files}"));
                for index in 5..files {
                    write(&m.first.root, &format!("notes/file-{index}"), b"private\n");
                }
                if files > 5 {
                    private_add(s, &m.first.root, "notes");
                    private_commit(s, &m.first.root);
                    s.private(&m.first.root)
                        .git(["push", "origin", "main"])
                        .succeeds();
                }
                let source_index = private_index(s, &m.first.root);
                assert_eq!(
                    source_index.iter().filter(|&&byte| byte == 0).count(),
                    files
                );
                write(&m.root, ".env.local", b"mine\n");
                let url = m.first.private_remote.to_str().unwrap();
                let words = if branch {
                    vec!["dupe", "clone", url, "-b", "main"]
                } else {
                    vec!["dupe", "clone", url]
                };
                let (output, runs) = run_traced(s.git(words).from(&m.root), &s.dir().join("trace"));
                assert_eq!(output.end, End::Code(0), "{output:?}");
                assert_eq!(fs::read(m.root.join(".env.local")).unwrap(), b"mine\n");
                assert_eq!(private_index(s, &m.root), source_index);
                for path in [
                    ".gitdupe",
                    "notes/a.md",
                    ".vscode/settings.json",
                    "docs/notes.md",
                ] {
                    assert_eq!(
                        fs::read(m.root.join(path)).unwrap(),
                        fs::read(m.first.root.join(path)).unwrap(),
                        "{path}"
                    );
                }
                for index in 5..files {
                    let path = format!("notes/file-{index}");
                    assert_eq!(
                        fs::read(m.root.join(&path)).unwrap(),
                        b"private\n",
                        "{path}"
                    );
                }
                let own = runs.own();
                let sequence: Vec<_> = own.commands().into_iter().map(<[u8]>::to_vec).collect();
                let expected: Vec<_> = [
                    "rev-parse",
                    "config",
                    "worktree",
                    "ls-files",
                    "symbolic-ref",
                    "init",
                    "config",
                    "config",
                    "config",
                    "config",
                    "config",
                    "config",
                    "remote",
                    "fetch",
                    "for-each-ref",
                    "ls-remote",
                    "branch",
                    "symbolic-ref",
                    "reset",
                    "ls-files",
                    "checkout-index",
                    "diff",
                    "ls-files",
                    "ls-files",
                    "check-ignore",
                ]
                .into_iter()
                .filter(|word| !branch || *word != "ls-remote")
                .map(|word| word.as_bytes().to_vec())
                .collect();
                assert_eq!(sequence, expected, "{own:?}");
                sequences.push(sequence);
            }
            assert_eq!(sequences[0], sequences[1]);
        }
    });
}

#[test]
fn clone_ignores_a_callers_index_and_leaves_both_public_indexes_unchanged() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let twin = s.dir().join("project-twin");
        s.public_clone(&m.first.root, &twin);
        for root in [&m.root, &twin] {
            write(root, ".env.local", b"mine\n");
        }
        let public_path = m.root.join(".git/index");
        let caller_path = m.root.join(".git/hook-index");
        fs::copy(&public_path, &caller_path).unwrap();
        // Only the hook's index tracks `.gitdupe`: using it for the public query
        // would refuse clone (G4), or report a conflict during settle (G6).
        write(&m.root, ".gitdupe", b"hook index only\n");
        s.git(["add", "--", ".gitdupe"])
            .from(&m.root)
            .variable("GIT_INDEX_FILE", &caller_path)
            .succeeds();
        fs::remove_file(m.root.join(".gitdupe")).unwrap();
        let public_listing = s.git(["ls-files", "-z"]).from(&m.root).succeeds();
        let caller_listing = s
            .git(["ls-files", "-z"])
            .from(&m.root)
            .variable("GIT_INDEX_FILE", &caller_path)
            .succeeds();
        assert_ne!(public_listing.stdout, caller_listing.stdout);
        assert!(
            caller_listing
                .stdout
                .split(|&byte| byte == 0)
                .any(|p| p == b".gitdupe")
        );
        assert!(
            !public_listing
                .stdout
                .split(|&byte| byte == 0)
                .any(|p| p == b".gitdupe")
        );
        let public_before = fs::read(&public_path).unwrap();
        let caller_before = fs::read(&caller_path).unwrap();
        let twin_index = twin.join(".git/index");
        let twin_before = fs::read(&twin_index).unwrap();
        assert_ne!(public_before, caller_before);

        let url = m.first.private_remote.to_str().unwrap();
        let ordinary = s.git(["dupe", "clone", url]).from(&twin).run();
        let inherited = s
            .git(["dupe", "clone", url])
            .from(&m.root)
            .variable("GIT_INDEX_FILE", &caller_path)
            .run();
        assert_eq!(ordinary.end, End::Code(0), "{ordinary:?}");
        assert_eq!(inherited.end, ordinary.end, "{inherited:?}");
        for level in ["fatal", "error", "hint"] {
            assert!(ordinary.lines(level).is_empty(), "{ordinary:?}");
            assert_eq!(
                inherited.lines(level),
                ordinary.lines(level),
                "{inherited:?}"
            );
        }
        let warnings = ordinary.lines("warning");
        assert_eq!(warnings.len(), 1, "{ordinary:?}");
        names(warnings[0], b".env.local");
        assert_eq!(inherited.lines("warning"), warnings, "{inherited:?}");
        assert_eq!(fs::read(&public_path).unwrap(), public_before);
        assert_eq!(fs::read(&caller_path).unwrap(), caller_before);
        assert_eq!(fs::read(&twin_index).unwrap(), twin_before);
        let ordinary_tree = Tree::relative(&twin).without(&[Path::new(".git")]);
        let inherited_tree = Tree::relative(&m.root).without(&[Path::new(".git")]);
        let changed = ordinary_tree.changed_in(&inherited_tree);
        assert!(changed.is_empty(), "changed: {changed:?}");
        assert_eq!(fs::read(m.root.join(".env.local")).unwrap(), b"mine\n");
        assert_eq!(region_rules(&m.root), region_rules(&twin));
        for root in [&m.root, &twin] {
            assert_eq!(private_index(s, root), private_index(s, &m.first.root));
            assert_eq!(private_head(s, root), private_head(s, &m.first.root));
            let staged = s.private(root).git(["diff", "--cached", "--quiet"]).run();
            assert_eq!(staged.end, End::Code(0), "{staged:?}");
        }
    });
}
