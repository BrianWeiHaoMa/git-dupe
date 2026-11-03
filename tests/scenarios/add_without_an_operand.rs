//! `git dupe add` given no path, and given a last option that lacks its value (G13, G26,
//! `Holds/G13, G14`, S4). With no operand and the all-flag clear the run is the user's words,
//! and where Git acts on nothing, one `hint:` names the private form of what its advice
//! names; a last word lacking its value is Git's usage error, nothing hidden for it; the
//! forms that find nothing hidden name both ways on; and a `.gitdupe` the project tracks
//! is refused saying where to run the command that releases it.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, dupe_succeeds, hinted, holds, names, private_alias,
    private_commit, region_rules, under_each_release, unhinted, write,
};

/// The commands the hint after an `add` that named nothing names.
const NOTHING_NAMED: [&[u8]; 2] = [
    b"'git dupe add .'",
    b"'git dupe config advice.addEmptyPathspec false'",
];

/// An attached project with a private `notes/a.md` committed, a public `src/scratch.py`
/// untracked, and no maintenance left running to disturb a comparison of runs.
fn workspace(s: &Scenario) -> PathBuf {
    s.git(["config", "--global", "maintenance.auto", "false"])
        .succeeds();
    let dir = s.dir().join("project");
    s.attached_project(&dir);
    write(&dir, "notes/a.md", b"a\n");
    dupe_succeeds(s, &dir, &["add", "notes/"]);
    private_commit(s, &dir);
    write(&dir, "src/scratch.py", b"scratch\n");
    dir
}

#[test]
fn an_add_that_names_nothing_ends_with_one_hint_and_changes_nothing() {
    under_each_release(|s| {
        let dir = workspace(s);
        private_alias(s, &dir, "a", "add");
        let public = s.git(["status", "--porcelain"]).from(&dir).succeeds();
        for words in [
            &["add"][..],
            &["add", "--refresh"],
            &["add", "--renormalize"],
            &["add", "-n"],
            &["add", "-v"],
            &["add", "--no-all"],
            &["a"],
        ] {
            let ours = hinted(s, &dir, words, &NOTHING_NAMED);
            assert_eq!(ours.end, End::Code(0), "{words:?}: {ours:?}");
            let after = s.git(["status", "--porcelain"]).from(&dir).succeeds();
            assert_eq!(after.stdout, public.stdout, "{words:?}");
        }
    });
}

#[test]
fn an_add_git_acts_on_without_a_path_gets_no_hint() {
    under_each_release(|s| {
        let dir = workspace(s);
        for words in [&["add", "-u"][..], &["add", "--update"], &["add", "-p"]] {
            let ours = unhinted(s, &dir, words);
            assert_eq!(ours.end, End::Code(0), "{words:?}: {ours:?}");
        }
        s.private(&dir)
            .git(["config", "core.editor", "true"])
            .succeeds();
        write(&dir, "notes/a.md", b"edited\n");
        let edited = unhinted(s, &dir, &["add", "-e"]);
        assert_eq!(edited.end, End::Code(0), "{edited:?}");

        // A help request is the text, and no run.
        let help = s.git(["dupe", "add", "-h"]).from(&dir).run();
        assert_eq!(help.end, End::Code(0), "{help:?}");
        assert!(help.lines("hint").is_empty(), "{help:?}");
    });
}

/// The value missing: Git's usage error, as plain `git add notes/ --chmod`, with nothing
/// hidden first, where a `--` of git-dupe's would have been taken for the value.
fn refused_by_git(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let gitdupe = fs::read(dir.join(".gitdupe")).unwrap();
    let rules = region_rules(dir);
    let index = fs::read(dir.join(".git/dupe/index")).unwrap();
    let output = s.git(["dupe"].iter().chain(words)).from(dir).run();
    assert_eq!(output.end, End::Code(129), "{words:?}: {output:?}");
    assert!(output.lines("hint").is_empty(), "{words:?}: {output:?}");
    assert!(output.lines("fatal").is_empty(), "{words:?}: {output:?}");
    assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), gitdupe);
    assert_eq!(region_rules(dir), rules);
    assert_eq!(fs::read(dir.join(".git/dupe/index")).unwrap(), index);
    output
}

#[test]
fn a_last_option_lacking_its_value_is_gits_usage_error_and_hides_nothing() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, "tools/run.sh", b"private\n");
        for words in [
            &["add", "tools/", "--chmod"][..],
            &["add", "tools", "-v", "--chmod"],
            &["add", ".", "--chmod"],
            &["add", "--chmod"],
        ] {
            refused_by_git(s, &dir, words);
        }

        // The value given is taken, as Git takes it.
        write(&dir, "notes/x.md", b"x\n");
        let chmod = ["add", "--chmod", "+x", "notes/x.md"];
        s.git(["dupe"].iter().chain(&chmod)).from(&dir).succeeds();
        let staged = s.private(&dir).git(["ls-files", "--stage", "notes/x.md"]);
        assert!(staged.succeeds().stdout.starts_with(b"100755 "));

        // A refusal holds first, as for the same directory with the value given.
        write(&dir, "docs/mine.md", b"mine\n");
        let refused = s.git(["dupe", "add", "docs/", "--chmod"]).from(&dir).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
    });
}

#[test]
fn a_directory_to_hide_is_refused_over_an_unreadable_list_though_its_last_option_lacks_a_value() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, "tools/run.sh", b"private\n");
        write(&dir, ".gitdupe", b"\xff\xfen\0o\0t\0e\0s\0\n\0");
        let gitdupe = fs::read(dir.join(".gitdupe")).unwrap();
        let rules = region_rules(&dir);

        // As `hide` would refuse it, the value given or not (F5, G13).
        for words in [&["add", "tools/"][..], &["add", "tools/", "--chmod"]] {
            let refused = s.git(["dupe"].iter().chain(words)).from(&dir).run();
            assert_eq!(refused.end, End::Code(128), "{words:?}: {refused:?}");
            let fatal = refused.lines("fatal");
            assert_eq!(fatal.len(), 1, "{words:?}: {refused:?}");
            names(fatal[0], b".gitdupe");
        }
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), gitdupe);
        assert_eq!(region_rules(&dir), rules);

        // With nothing to hide, nothing is refused, and the words are Git's to refuse.
        refused_by_git(s, &dir, &["add", "notes/a.md", "--chmod"]);
    });
}

#[test]
fn a_form_finding_nothing_hidden_names_both_ways_on() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, "notes/a.md", b"a\n");
        dupe_succeeds(s, &dir, &["add", "notes/"]);
        private_commit(s, &dir);
        dupe_succeeds(s, &dir, &["rm", "-q", "--cached", "notes/a.md"]);
        private_commit(s, &dir);
        write(&dir, ".gitdupe", b"");

        let before = Tree::of(&dir.join(".git/dupe"));
        let output = s.git(["dupe", "add", "."]).from(&dir).succeeds();
        let hints = output.lines("hint");
        assert_eq!(hints.len(), 1, "{output:?}");
        let hint = hints[0];
        names(hint, b"'git dupe add <path>'");
        names(hint, b"'git dupe add .gitdupe'");
        names(hint, b"run from the root");
        assert!(
            before
                .changed_in(&Tree::of(&dir.join(".git/dupe")))
                .is_empty()
        );

        // The second way on, which stages the list, is named only where it can.
        s.git(["dupe", "add", ".gitdupe"]).from(&dir).succeeds();
        let staged = s.private(&dir).git(["show", ":.gitdupe"]).succeeds();
        assert!(staged.stdout.is_empty(), "{staged:?}");
    });
}

#[test]
fn a_list_the_private_repository_does_not_track_is_not_offered_to_add() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let output = s.git(["dupe", "add", "."]).from(&dir).succeeds();
        let hint = output.only_line("hint");
        names(hint, b"'git dupe add <path>'");
        assert!(!holds(hint, b"add .gitdupe"), "{output:?}");
    });
}

#[test]
fn a_public_gitdupe_is_refused_saying_where_to_run_the_command_that_releases_it() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"notes\n");
        s.git(["add", "-f", ".gitdupe"]).from(&dir).succeeds();
        s.commit_public(&dir);
        write(&dir, "src/tools/x", b"x\n");
        let from = dir.join("src");
        for words in [
            &["dupe", "hide", "../x"][..],
            &["dupe", "unhide", "x"],
            &["dupe", "add", "tools/"],
        ] {
            let output = s.git(words).from(&from).run();
            assert_eq!(output.end, End::Code(128), "{words:?}: {output:?}");
            let fatal = output.lines("fatal");
            assert_eq!(fatal.len(), 1, "{output:?}");
            names(fatal[0], b"'git rm --cached .gitdupe' from the root");
        }
    });
}
