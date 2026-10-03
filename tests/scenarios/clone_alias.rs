//! Aliases reaching `clone` attach with their expanded arguments (G19), read global
//! rather than public aliases when unattached (`Holds/G4`), locate the superproject
//! (`Composition/Runner`), and answer clone help even outside a repository (G24).

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, locate_words, names, private_add, private_commit, region_rules,
    run_traced, unchanged, under_each_release, write,
};

/// Configures a global alias in the scenario's isolated home.
fn alias(s: &Scenario, name: &str, expansion: &str) {
    s.git(["config", "--global", &format!("alias.{name}"), expansion])
        .succeeds();
}

/// The Git answer for the private repository at `root`.
fn private_answer(s: &Scenario, root: &Path, words: &[&str]) -> Vec<u8> {
    s.private(root).git(words).succeeds().stdout
}

/// Compares only git-dupe's lines, leaving Git's own messages to Git.
fn same_own_lines(aliased: &Output, typed: &Output) {
    for level in ["fatal", "error", "warning", "hint"] {
        assert_eq!(aliased.lines(level), typed.lines(level), "{level}");
    }
}

#[test]
fn a_global_clone_alias_attaches_as_typed_and_locates_the_superproject() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let twin = s.dir().join("project-twin");
        s.public_clone(&m.first.root, &twin);
        alias(s, "bring-private", "clone");
        for root in [&m.root, &twin] {
            // Public configuration must not override the global alias when unattached.
            s.git(["config", "--local", "alias.bring-private", "init"])
                .from(root)
                .succeeds();
            write(root, ".env.local", b"this machine's settings\n");
        }
        let url = m.first.private_remote.to_str().unwrap();
        let typed = s.git(["dupe", "clone", url]).from(&twin).run();
        let (aliased, runs) = run_traced(
            s.git(["dupe", "bring-private", url]).from(&m.root),
            &s.dir().join("alias-trace"),
        );
        assert_eq!(typed.end, End::Code(0), "{typed:?}");
        assert_eq!(aliased.end, typed.end, "{aliased:?}");
        same_own_lines(&aliased, &typed);
        let warnings = aliased.lines("warning");
        assert_eq!(warnings.len(), 1, "{aliased:?}");
        names(warnings[0], b".env.local");
        let locate: Vec<Vec<u8>> = locate_words(true)
            .iter()
            .map(|word| word.as_bytes().to_vec())
            .collect();
        let own = runs.own();
        assert!(own.words().iter().any(|words| words == &locate), "{runs:?}");

        let working = |root: &Path| Tree::relative(root).without(&[Path::new(".git")]);
        let changed = working(&m.root).changed_in(&working(&twin));
        assert!(changed.is_empty(), "changed: {changed:?}");
        assert_eq!(region_rules(&m.root), region_rules(&twin));
        for words in [
            &["ls-files", "--stage", "-z"][..],
            &["config", "--local", "--list", "-z"],
            &["symbolic-ref", "HEAD"],
            &["rev-parse", "HEAD"],
        ] {
            assert_eq!(
                private_answer(s, &m.root, words),
                private_answer(s, &twin, words),
                "{words:?}"
            );
        }
    });
}

#[test]
fn a_global_clone_alias_answers_h_from_a_second_machine_and_outside() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        alias(s, "bring-private", "clone");
        let help = s.git(["dupe", "help", "clone"]).succeeds();
        assert_eq!(help.end, End::Code(0));
        for root in [&m.root, s.dir()] {
            let before = Tree::of(root);
            let output = s.git(["dupe", "bring-private", "-h"]).from(root).run();
            assert_eq!(output, help, "{}", root.display());
            unchanged(&before, root);
        }
    });
}

#[test]
fn a_global_clone_alias_keeps_b_other_before_the_typed_url() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let first = &m.first.root;
        s.private(first)
            .git(["checkout", "-q", "-b", "other"])
            .succeeds();
        write(first, "notes/other.md", b"only on other\n");
        private_add(s, first, "notes/other.md");
        private_commit(s, first);
        s.private(first)
            .git(["push", "-q", "origin", "other"])
            .succeeds();
        alias(s, "bring-other", "clone -b other");
        let url = m.first.private_remote.to_str().unwrap();
        let output = s.git(["dupe", "bring-other", url]).from(&m.root).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        for level in ["fatal", "error", "warning", "hint"] {
            assert!(output.lines(level).is_empty(), "{output:?}");
        }
        assert_eq!(
            private_answer(s, &m.root, &["symbolic-ref", "HEAD"]),
            b"refs/heads/other\n"
        );
        assert_eq!(
            private_answer(
                s,
                &m.root,
                &["rev-parse", "--symbolic-full-name", "other@{upstream}"]
            ),
            b"refs/remotes/origin/other\n"
        );
        assert_eq!(
            fs::read(m.root.join("notes/other.md")).unwrap(),
            b"only on other\n"
        );
    });
}
