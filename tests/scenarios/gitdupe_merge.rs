//! Two private branches that each hid a path merge, and rebase, without a conflict in
//! `.gitdupe`: Git's own `union` driver, which F2's line in the private repository's
//! `info/attributes` names, keeps each side's added line (N5). A path one branch alone
//! unhid is unhidden by the merge, as any one-sided change is, and the command names what
//! that leaves visible (G9).

use std::path::Path;

use crate::harness::{
    End, Output, Scenario, daily_state, gitdupe_written_and_staged, names, private_commit,
    region_rules, under_each_release, write,
};

/// `git dupe <words>` from `dir` with an identity for any commit it makes.
fn dupe(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let identity = [
        "-c",
        "user.name=Scenario",
        "-c",
        "user.email=scenario@example.invalid",
        "-c",
        "maintenance.auto=false",
        "dupe",
    ];
    let words: Vec<&str> = identity.iter().chain(words).copied().collect();
    s.git(&words).from(dir).run()
}

/// `daily_state` at `name`, listing `notes` and `.vscode`, with the branch `one` hiding
/// `one` and `main` hiding `two`, each committed, `main` checked out.
fn diverged(s: &Scenario, name: &str) -> std::path::PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    for (branch, path) in [("one", "one"), ("main", "two")] {
        if branch == "one" {
            assert_eq!(dupe(s, &dir, &["switch", "-qc", "one"]).end, End::Code(0));
        } else {
            assert_eq!(dupe(s, &dir, &["switch", "-q", "main"]).end, End::Code(0));
        }
        assert_eq!(dupe(s, &dir, &["hide", path]).end, End::Code(0));
        private_commit(s, &dir);
    }
    dir
}

/// The list holds no conflict marker and each of `paths` once, in any order; the region
/// holds each path's rule; and a later `hide` stages the list with those lines as they are.
fn merged_whole(s: &Scenario, dir: &Path, paths: &[&str]) {
    let list = std::fs::read(dir.join(".gitdupe")).unwrap();
    let mut lines: Vec<&[u8]> = list
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    lines.sort();
    let mut expected: Vec<&[u8]> = paths.iter().map(|path| path.as_bytes()).collect();
    expected.sort();
    assert_eq!(lines, expected, "{}", list.escape_ascii());
    let rules = region_rules(dir);
    for path in paths {
        let rule = [b"/", path.as_bytes()].concat();
        assert!(rules.contains(&rule), "{path}: {rules:?}");
    }
    assert_eq!(dupe(s, dir, &["hide", "three"]).end, End::Code(0));
    gitdupe_written_and_staged(s, dir, &[&list[..], b"three\n"].concat());
}

#[test]
fn two_branches_that_each_hid_a_path_merge_and_rebase_without_a_conflict() {
    under_each_release(|s| {
        let dir = diverged(s, "merged");
        let merge = dupe(s, &dir, &["merge", "--no-edit", "one"]);
        assert_eq!(merge.end, End::Code(0), "{merge:?}");
        let unmerged = s.private(&dir).git(["ls-files", "-u"]).succeeds();
        assert!(unmerged.stdout.is_empty(), "{unmerged:?}");
        merged_whole(s, &dir, &["notes", ".vscode", "one", "two"]);

        let dir = diverged(s, "rebased");
        assert_eq!(dupe(s, &dir, &["switch", "-q", "one"]).end, End::Code(0));
        let rebase = dupe(s, &dir, &["rebase", "main"]);
        assert_eq!(rebase.end, End::Code(0), "{rebase:?}");
        assert!(!dir.join(".git/dupe/rebase-merge").exists(), "{rebase:?}");
        merged_whole(s, &dir, &["notes", ".vscode", "one", "two"]);
    });
}

/// A branch that unhid `scratch`, merged into `main`, which left the list alone and moved
/// on elsewhere, unhides it, and the merge names `scratch` as visible to the project's Git.
#[test]
fn a_path_one_branch_alone_unhid_is_unhidden_by_the_merge_and_named() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        assert_eq!(dupe(s, &dir, &["hide", "scratch"]).end, End::Code(0));
        private_commit(s, &dir);
        assert_eq!(dupe(s, &dir, &["switch", "-qc", "drop"]).end, End::Code(0));
        assert_eq!(dupe(s, &dir, &["unhide", "scratch"]).end, End::Code(0));
        private_commit(s, &dir);
        assert_eq!(dupe(s, &dir, &["switch", "-q", "main"]).end, End::Code(0));
        write(&dir, "notes/b.md", b"elsewhere\n");
        assert_eq!(dupe(s, &dir, &["add", "notes/b.md"]).end, End::Code(0));
        private_commit(s, &dir);
        write(&dir, "scratch/x.md", b"draft\n");

        let merge = dupe(s, &dir, &["merge", "--no-edit", "drop"]);
        assert_eq!(merge.end, End::Code(0), "{merge:?}");
        assert_eq!(
            std::fs::read(dir.join(".gitdupe")).unwrap(),
            b"notes\n.vscode\n"
        );
        let warnings = merge.lines("warning");
        assert_eq!(warnings.len(), 1, "{merge:?}");
        names(warnings[0], b"scratch");
        assert!(!region_rules(&dir).contains(&b"/scratch".to_vec()));
    });
}
