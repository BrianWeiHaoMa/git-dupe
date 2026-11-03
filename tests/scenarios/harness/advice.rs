//! The hint git-dupe writes after a Git run of the user's words whose outcome Git's own
//! advice names a plain `git` command for (`Holds/G19`, `Holds/G18`, G25): the words run
//! three times from one state at one path, as the release's own Git against the private
//! repository, as `git dupe git`, and as `git dupe`, and git-dupe's output compared with
//! Git's. A scenario names no Git text: the state it builds decides what Git prints.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::executable::write_executable;
use super::files::copy;
use super::fresh::FreshDirectory;
use super::output::{End, Output, holds, names};
use super::platform::as_git_spells;
use super::scenario::{Git, Scenario};
use super::trace::{run_traced, run_words};
use super::tree::Tree;

/// The commands the hint after an add of a path the private repository ignores names.
pub const IGNORED_HINT: [&[u8]; 2] = [
    b"'git dupe add -f <path>'",
    b"'git dupe config advice.addIgnoredFile false'",
];

/// The commands the hint after a private rebase that stopped names, at a conflict, an
/// `edit` or `break` stop, a reword whose editor failed, or a todo list Git could not read.
pub const STOPPED_REBASE_HINT: [&[u8]; 7] = [
    b"'git dupe add <path>'",
    b"'git dupe commit --amend'",
    b"'git dupe commit --allow-empty'",
    b"'git dupe rebase --edit-todo'",
    b"'git dupe rebase --continue'",
    b"'git dupe rebase --skip'",
    b"'git dupe rebase --abort'",
];

/// Makes the private sequence editor of the workspace at `root` the script `name`, which
/// rewrites the todo list it is given through `program`, a `sed` or `awk` program.
pub fn sequence_editor(s: &Scenario, root: &Path, name: &str, program: &str) {
    let script = s.dir().join(name);
    let body = format!("#!/bin/sh\n{program} \"$1\" > \"$1.new\" && mv \"$1.new\" \"$1\"\n");
    write_executable(s, &script, body.as_bytes());
    s.private(root)
        .git(["config", "sequence.editor", &as_git_spells(&script)])
        .succeeds();
}

/// The workspace at `root` copied aside, to be put back at the same path, so that the
/// paths Git's output names are the same in every execution.
struct Saved {
    root: PathBuf,
    tree: Tree,
    aside: FreshDirectory,
}

impl Saved {
    fn of(root: &Path) -> Saved {
        let aside = FreshDirectory::create();
        copy(root, &aside.path().join("workspace"));
        Saved {
            root: root.to_path_buf(),
            tree: Tree::of(root),
            aside,
        }
    }

    /// Puts the copy back where a run changed anything below the root, so that a scenario
    /// that checks afterwards that words changed nothing observes every execution.
    fn put_back(&self) {
        if self.tree.changed_in(&Tree::of(&self.root)).is_empty() {
            return;
        }
        fs::remove_dir_all(&self.root)
            .unwrap_or_else(|cause| panic!("{}: {cause}", self.root.display()));
        copy(&self.aside.path().join("workspace"), &self.root);
    }
}

/// How git-dupe ends after a Git child that ended as `end`: with its status, or with 128
/// plus the signal that killed it.
fn as_git_dupe_ends(end: End) -> End {
    match end {
        End::Signal(signal) => End::Code(128 + signal),
        code => code,
    }
}

/// Turns Git's automatic maintenance off for the scenario, before its workspace is made:
/// a release that runs it detached, as 2.56.0 does after `fetch` or `commit`, leaves it
/// writing below `.git` after the run has returned, while `hinted` and `unhinted` read
/// the workspace and put it back.
pub fn without_maintenance(s: &Scenario) {
    s.git(["config", "--global", "maintenance.auto", "false"])
        .succeeds();
}

/// `git dupe <words>` from the root of the workspace at `root`, and Git's own run of the
/// words against its private repository after `-c help.autocorrect=0`, as git-dupe runs
/// them, from the same state: the workspace is put back between the runs, and
/// `git dupe git <words>`, guarded by nothing, ends as Git's own run (G20). The words may
/// change only what lies below the root; the scenario goes on from the state `git dupe`
/// left. Automatic maintenance must be off in both repositories (`without_maintenance`).
/// The three runs date the commits they make at one moment, so that a commit Git names by
/// its hash, as the stash entry of an autostash, is the same commit in each.
fn ours_and_gits(s: &Scenario, root: &Path, words: &[&str]) -> (Output, Output) {
    // Required rather than set here, so that a scenario missing it fails every time
    // instead of only when Git's background maintenance races the reading, in either
    // repository, both of which lie below the root that `Saved` copies.
    let asked = ["config", "--type=bool", "--get", "maintenance.auto"];
    for (repository, maintenance) in [
        ("public", s.git(asked).from(root).run()),
        ("private", s.private(root).git(asked).run()),
    ] {
        assert_eq!(
            maintenance.stdout, b"false\n",
            "a scenario comparing runs turns automatic maintenance off first, in the \
             {repository} repository too: {maintenance:?}"
        );
    }
    let saved = Saved::of(root);
    let since_the_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let moment = format!("@{} +0000", since_the_epoch.as_secs());
    let at_the_moment = |git: Git<'_>| {
        git.variable("GIT_AUTHOR_DATE", &moment)
            .variable("GIT_COMMITTER_DATE", &moment)
            .run()
    };
    let mut gits = at_the_moment(
        s.private(root).git(
            ["-c", "help.autocorrect=0"]
                .into_iter()
                .chain(words.iter().copied()),
        ),
    );
    gits.end = as_git_dupe_ends(gits.end);
    saved.put_back();
    let unguarded = at_the_moment(
        s.git(["dupe", "git"].into_iter().chain(words.iter().copied()))
            .from(root),
    );
    assert_eq!(unguarded, gits, "git dupe git {words:?}");
    saved.put_back();
    let ours = at_the_moment(
        s.git(["dupe"].into_iter().chain(words.iter().copied()))
            .from(root),
    );
    (ours, gits)
}

/// `git dupe <words>` ends as Git's own run of them, with every byte of its output, and
/// then one `hint:` line naming each of `parts`, before anything settle adds; its output.
pub fn hinted(s: &Scenario, root: &Path, words: &[&str], parts: &[&[u8]]) -> Output {
    let (ours, gits) = ours_and_gits(s, root, words);
    gits_then_one_hint(&ours, &gits, parts);
    ours
}

/// `ours` ends as `gits`, with every byte of its output, and then one `hint:` line naming
/// each of `parts`, nothing after it: for a scenario that makes Git's own run itself.
pub fn gits_then_one_hint(ours: &Output, gits: &Output, parts: &[&[u8]]) {
    assert_eq!(ours.end, gits.end, "{ours:?}; Git: {gits:?}");
    assert_eq!(ours.stdout, gits.stdout, "{ours:?}; Git: {gits:?}");
    let hint = ours
        .stderr
        .strip_prefix(gits.stderr.as_slice())
        .and_then(|after| after.strip_prefix(b"hint: "))
        .and_then(|after| after.strip_suffix(b"\n"))
        .filter(|line| !line.contains(&b'\n'))
        .unwrap_or_else(|| panic!("not Git's output and one hint: {ours:?}; Git: {gits:?}"));
    for part in parts {
        names(hint, part);
    }
}

/// `git dupe <words>` ends as Git's own run of them, with every byte of its output and
/// nothing of git-dupe's; its output.
pub fn unhinted(s: &Scenario, root: &Path, words: &[&str]) -> Output {
    let (ours, gits) = ours_and_gits(s, root, words);
    assert_eq!(ours, gits, "{words:?}");
    ours
}

/// `output` ended with status 0 and holds nothing of git-dupe's: no line of Git's names
/// `git dupe`.
pub fn succeeded_without_a_hint(output: &Output) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(!holds(&output.stderr, b"git dupe"), "{output:?}");
}

/// `git dupe <words>` from the root of the workspace at `root`, `words` beginning with a
/// command word passed through, ends as `ended`, and the Git runs it started itself are, in
/// order, the locate, the alias lookup of that word, the run of the words, `questions`,
/// each with exactly its words, and settle's three: the questions its hint put to the
/// private repository, fixed for the words and the end (G22).
pub fn asked_after(s: &Scenario, root: &Path, words: &[&str], ended: End, questions: &[&[&str]]) {
    let (output, runs) = run_traced(
        s.git(["dupe"].iter().chain(words)).from(root),
        &s.dir().join("trace"),
    );
    assert_eq!(output.end, ended, "{words:?}: {output:?}");
    let runs = runs.own();
    let mut commands = vec![&b"rev-parse"[..], b"config", words[0].as_bytes()];
    commands.extend(questions.iter().map(|question| question[0].as_bytes()));
    commands.extend([&b"ls-files"[..], b"ls-files", b"check-ignore"]);
    assert_eq!(runs.commands(), commands, "{words:?}");
    for (at, question) in questions.iter().enumerate() {
        assert_eq!(runs.words()[3 + at], run_words(question), "{words:?}");
    }
}

/// `git dupe <words>` from the root of the workspace at `root`, `words` beginning with
/// `push` or `pull`, ends as `ended`, and the Git runs it started itself are, in order,
/// the locate, the places and the private configuration its guard reads, `HEAD` where
/// `reads_head`, the run of the words, `questions`, each with exactly its words, and
/// settle's three: the questions its hint put to the private repository, fixed for the
/// words and the end (G22).
pub fn transfer_asked_after(
    s: &Scenario,
    root: &Path,
    words: &[&str],
    ended: End,
    reads_head: bool,
    questions: &[&[&str]],
) {
    let (output, runs) = run_traced(
        s.git(["dupe"].iter().chain(words)).from(root),
        &s.dir().join("trace"),
    );
    assert_eq!(output.end, ended, "{words:?}: {output:?}");
    let runs = runs.own();
    let mut commands = vec![&b"rev-parse"[..], b"config", b"worktree", b"config"];
    if reads_head {
        commands.push(b"symbolic-ref");
    }
    commands.push(words[0].as_bytes());
    let asked = commands.len();
    commands.extend(questions.iter().map(|question| question[0].as_bytes()));
    commands.extend([&b"ls-files"[..], b"ls-files", b"check-ignore"]);
    assert_eq!(runs.commands(), commands, "{words:?}");
    for (at, question) in questions.iter().enumerate() {
        assert_eq!(runs.words()[asked + at], run_words(question), "{words:?}");
    }
}
