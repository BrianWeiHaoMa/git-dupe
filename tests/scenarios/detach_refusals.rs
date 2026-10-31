//! What `git dupe detach` refuses, and that a refusal changes nothing anywhere.

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Everything, Output, Scenario, Transfer, Tree, as_git_spells, detached, dupe_succeeds,
    holds, names, names_path, private_add, private_commit, refusal, region, run_traced,
    stale_file_timestamp, stale_timestamp, symlink, unchanged, under_each_release, write,
};

fn before_refusal(transfer: &Transfer) -> Everything {
    stale_timestamp(&transfer.root);
    transfer.everything()
}

fn refused(s: &Scenario, transfer: &Transfer) -> Vec<u8> {
    refused_as(transfer, || {
        s.git(["dupe", "detach"]).from(&transfer.root).run()
    })
}

/// `refused`, with `detach` the run `detach` makes: under a global option or a variable.
fn refused_as(transfer: &Transfer, detach: impl FnOnce() -> Output) -> Vec<u8> {
    let before = before_refusal(transfer);
    let output = detach();
    let line = refusal(&output, &[b"git dupe detach --force"]).to_vec();
    before.unchanged();
    assert!(transfer.root.join(".git/dupe").is_dir());
    assert!(region(&transfer.root).is_some());
    line
}

fn clean(s: &Scenario, root: &Path) {
    let status = s
        .private(root)
        .git([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=no",
        ])
        .succeeds();
    assert!(status.stdout.is_empty(), "{status:?}");
}

#[test]
fn uncommitted_modification_addition_and_deletion() {
    under_each_release(|s| {
        let mut lines = Vec::new();
        for cause in ["modified", "added", "deleted"] {
            let transfer = s.pushed_workspace(cause);
            let root = &transfer.root;
            match cause {
                "modified" => write(root, "notes/a.md", b"changed\n"),
                "added" => {
                    write(root, "notes/new.md", b"new\n");
                    private_add(s, root, "notes/new.md");
                }
                "deleted" => {
                    s.private(root)
                        .git(["rm", "--cached", "--", "notes/a.md"])
                        .succeeds();
                }
                _ => unreachable!(),
            }
            let line = refused(s, &transfer);
            names(&line, b"not committed");
            lines.push(line);
        }
        assert!(lines.windows(2).all(|pair| pair[0] == pair[1]));

        let unpushed = s.pushed_workspace("unpushed");
        write(&unpushed.root, "notes/a.md", b"committed\n");
        private_add(s, &unpushed.root, "notes/a.md");
        private_commit(s, &unpushed.root);
        let line = refused(s, &unpushed);
        names(&line, b"no remote-tracking branch");
        assert_ne!(line, lines[0]);
    });
}

#[test]
fn unpushed_stash_tag_and_second_branch() {
    under_each_release(|s| {
        let mut lines = Vec::new();
        for cause in ["stash", "older-stash", "tag", "branch"] {
            let transfer = s.pushed_workspace(cause);
            let root = &transfer.root;
            if cause == "stash" {
                stash(s, root, b"stashed\n");
            } else if cause == "older-stash" {
                // Every stash entry counts, not only the newest: here the newest is on a
                // remote-tracking branch and the older one alone is on none.
                stash(s, root, b"older\n");
                stash(s, root, b"newest\n");
                s.private(root)
                    .git(["push", "-q", "origin", "refs/stash:refs/heads/kept"])
                    .succeeds();
                s.private(root).git(["fetch", "-q", "origin"]).succeeds();
                let unpushed = |entry: &str| {
                    let found = s
                        .private(root)
                        .git(["rev-list", "-n", "1", entry, "--not", "--remotes"])
                        .succeeds();
                    !found.stdout.is_empty()
                };
                assert!(!unpushed("stash@{0}"));
                assert!(unpushed("stash@{1}"));
            } else {
                let checkout = if cause == "tag" {
                    ["checkout", "--detach", "main"]
                } else {
                    ["checkout", "-b", "second"]
                };
                s.private(root).git(checkout).succeeds();
                write(root, "notes/a.md", b"committed\n");
                private_add(s, root, "notes/a.md");
                private_commit(s, root);
                if cause == "tag" {
                    s.private(root).git(["tag", "only-tag"]).succeeds();
                }
                s.private(root).git(["checkout", "main"]).succeeds();
            }
            clean(s, root);
            let line = refused(s, &transfer);
            names(&line, b"no remote-tracking branch");
            lines.push(line);
        }
        assert!(lines.windows(2).all(|pair| pair[0] == pair[1]));
    });
}

/// Stashes a change of `notes/a.md` to `content`, leaving the tree clean.
fn stash(s: &Scenario, root: &Path, content: &[u8]) {
    write(root, "notes/a.md", content);
    s.private(root)
        .git([
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "stash",
            "push",
            "-q",
        ])
        .succeeds();
}

#[test]
fn no_remote_counts_commits_even_with_stale_remote_tracking_refs() {
    under_each_release(|s| {
        let removed = s.pushed_workspace("removed");
        s.private(&removed.root)
            .git(["config", "--remove-section", "remote.origin"])
            .succeeds();
        s.private(&removed.root)
            .git(["show-ref", "--verify", "refs/remotes/origin/main"])
            .succeeds();
        clean(s, &removed.root);
        let removed_line = refused(s, &removed);
        names(&removed_line, b"no remote is configured");

        let never = s.transfer_workspace("never-configured");
        clean(s, &never.root);
        let never_line = refused(s, &never);
        assert_eq!(removed_line, never_line);

        let pushed = s.pushed_workspace("pushed");
        s.git(["dupe", "detach"]).from(&pushed.root).succeeds();
        assert!(!pushed.root.join(".git/dupe").exists());
    });
}

/// The commit a detached `HEAD` names is on no private branch, tag, or stash entry, and
/// counts all the same (G3): refused while no remote-tracking branch holds it, with no
/// remote configured even when no branch is left, and detached once one does, or under
/// `--force`.
#[test]
fn the_commit_a_detached_head_names_counts() {
    under_each_release(|s| {
        let at_tip = s.pushed_workspace("detached-at-tip");
        dupe_succeeds(s, &at_tip.root, &["switch", "--detach", "main"]);
        detaches_keeping_every_file(s, &at_tip.root, &["detach"]);

        for cause in ["detached-commit", "detached-forced"] {
            let transfer = s.pushed_workspace(cause);
            let root = &transfer.root;
            dupe_succeeds(s, root, &["switch", "--detach"]);
            write(root, "notes/a.md", b"committed\n");
            private_add(s, root, "notes/a.md");
            private_commit(s, root);
            clean(s, root);
            if cause == "detached-forced" {
                detaches_keeping_every_file(s, root, &["detach", "--force"]);
            } else {
                names(&refused(s, &transfer), b"no remote-tracking branch");
                dupe_succeeds(s, root, &["push", "-q", "origin", "HEAD:refs/heads/kept"]);
                dupe_succeeds(s, root, &["fetch", "-q", "origin"]);
                detaches_keeping_every_file(s, root, &["detach"]);
            }
        }

        let never = s.transfer_workspace("detached-without-remote");
        dupe_succeeds(s, &never.root, &["switch", "--detach", "main"]);
        dupe_succeeds(s, &never.root, &["branch", "-D", "main"]);
        clean(s, &never.root);
        names(&refused(s, &never), b"no remote is configured");
    });
}

/// A way the private repository's own view grafts commits, which no remote holds (G3).
#[derive(Clone, Copy, Debug)]
enum Graft {
    /// A replace ref, `git replace --graft`.
    ReplaceRef,
    /// A line of the grafts file, which every run that parses a commit reads, with Git's
    /// advice that grafts are deprecated unless the configuration turns it off.
    GraftsFile,
}

const GRAFTS: [Graft; 2] = [Graft::ReplaceRef, Graft::GraftsFile];

const GRAFTS_FILE: &str = ".git/dupe/info/grafts";

/// The global options a caller puts before `detach`, and a variable it sets for the run.
type Caller = (&'static [&'static str], Option<(&'static str, OsString)>);

impl Graft {
    /// Gives `commit` the parents `parents` in the view of the private repository at
    /// `root`; a grafts line with Git's advice on it off there, as the advice says, so
    /// that `detach`'s other runs, which read the file, add no line of Git's either.
    fn graft(self, s: &Scenario, root: &Path, commit: &str, parents: &[&str]) {
        let words: Vec<&str> = [commit]
            .into_iter()
            .chain(parents.iter().copied())
            .collect();
        match self {
            Graft::ReplaceRef => {
                dupe_succeeds(s, root, &[&["replace", "--graft"], &words[..]].concat());
            }
            Graft::GraftsFile => {
                write(
                    root,
                    GRAFTS_FILE,
                    format!("{}\n", words.join(" ")).as_bytes(),
                );
                s.private(root)
                    .git(["config", "advice.graftFileDeprecated", "false"])
                    .succeeds();
            }
        }
    }

    /// Takes the graft of `commit` away again.
    fn ungraft(self, s: &Scenario, root: &Path, commit: &str) {
        match self {
            Graft::ReplaceRef => {
                dupe_succeeds(s, root, &["replace", "-d", commit]);
            }
            Graft::GraftsFile => fs::remove_file(root.join(GRAFTS_FILE)).unwrap(),
        }
    }

    /// Whether `name` is on no remote-tracking branch of the private repository at `root`:
    /// as `git dupe` walks it, which reads the graft as Git does, or, when `aside`, as Git
    /// walks the history as stored.
    fn unpushed(self, s: &Scenario, root: &Path, name: &str, aside: bool) -> bool {
        let words = ["rev-list", "-n", "1", name, "--not", "--remotes"];
        let walk = match (self, aside) {
            (_, false) => s.git(["dupe"].iter().chain(&words)).from(root),
            (Graft::ReplaceRef, true) => s
                .private(root)
                .git(["--no-replace-objects"].iter().chain(&words)),
            (Graft::GraftsFile, true) => s
                .private(root)
                .git(words)
                .variable("GIT_GRAFT_FILE", "/dev/null"),
        };
        !walk.succeeds().stdout.is_empty()
    }

    /// What a caller puts before `detach` to set this graft aside or keep it in force:
    /// global options, and a variable.
    fn callers(self, root: &Path) -> Vec<Caller> {
        match self {
            Graft::ReplaceRef => vec![
                (["--no-replace-objects", "dupe"].as_slice(), None),
                (&["dupe"], Some(("GIT_NO_REPLACE_OBJECTS", "1".into()))),
                (&["-c", "core.useReplaceRefs=true", "dupe"], None),
            ],
            Graft::GraftsFile => vec![(
                ["dupe"].as_slice(),
                Some(("GIT_GRAFT_FILE", root.join(GRAFTS_FILE).into())),
            )],
        }
    }
}

/// The history as stored decides (G3): a commit that a replace ref or a grafts line alone
/// grafts under the pushed `main` is on no remote-tracking branch, whatever a caller puts
/// before `dupe`, until it is pushed itself; `git dupe` itself walks the graft as Git
/// does; and the walk that decides adds no line of Git's to the refusal, whatever a
/// caller sets of Git's advice on grafts.
#[test]
fn a_commit_grafted_under_pushed_history_is_still_unpushed() {
    under_each_release(|s| {
        for graft in GRAFTS {
            let transfer = s.pushed_workspace(&format!("grafted-{graft:?}"));
            let root = &transfer.root;
            let main = commit_of(s, root, "main");
            s.private(root)
                .git(["switch", "-q", "--orphan", "rescue"])
                .succeeds();
            write(root, "unique", b"only here\n");
            private_add(s, root, "unique");
            private_commit(s, root);
            let rescue = commit_of(s, root, "rescue");
            s.private(root).git(["switch", "-q", "main"]).succeeds();
            graft.graft(s, root, &main, &[&rescue]);
            assert!(!graft.unpushed(s, root, "rescue", false), "{graft:?}");
            assert!(graft.unpushed(s, root, "rescue", true), "{graft:?}");
            clean(s, root);

            let line = refused(s, &transfer);
            names(&line, b"no remote-tracking branch");
            for (global, variable) in graft.callers(root) {
                let again = refused_as(&transfer, || {
                    let detach = s.git(global.iter().chain(&["detach"])).from(root);
                    match &variable {
                        Some((name, value)) => detach.variable(name, value).run(),
                        None => detach.run(),
                    }
                });
                assert_eq!(again, line, "{graft:?} {global:?} {variable:?}");
            }
            if let Graft::GraftsFile = graft {
                // The runs before the walk read the grafts file as Git does, and Git's
                // advice on it, turned on again, reaches standard error before the refusal.
                let before = before_refusal(&transfer);
                let output = s
                    .git(["-c", "advice.graftFileDeprecated=true", "dupe", "detach"])
                    .from(root)
                    .run();
                let fatal = [b"fatal: ", &line[..], b"\n"].concat();
                let gits = output.stderr.strip_suffix(&fatal[..]);
                assert!(gits.is_some_and(|gits| !gits.is_empty()), "{output:?}");
                assert_eq!(output.end, End::Code(128), "{output:?}");
                before.unchanged();
            }

            graft.ungraft(s, root, &main);
            for global in [
                ["dupe"].as_slice(),
                &["-c", "advice.graftFileDeprecated=true", "dupe"],
            ] {
                let again = refused_as(&transfer, || {
                    s.git(global.iter().chain(&["detach"])).from(root).run()
                });
                assert_eq!(again, line, "{graft:?} {global:?}");
            }
            dupe_succeeds(s, root, &["push", "-q", "origin", "rescue"]);
            graft.graft(s, root, &main, &[&rescue]);
            detaches_keeping_every_file(s, root, &["detach"]);
        }
    });
}

/// The other direction (G3): a pushed commit is on its remote-tracking branch as stored,
/// although a replace ref or a grafts line cuts it away from the pushed commit above it
/// here.
#[test]
fn a_pushed_commit_a_graft_cuts_away_is_still_pushed() {
    under_each_release(|s| {
        for graft in GRAFTS {
            let transfer = s.pushed_workspace(&format!("cut-{graft:?}"));
            let root = &transfer.root;
            dupe_succeeds(s, root, &["branch", "old"]);
            write(root, "notes/a.md", b"second\n");
            private_add(s, root, "notes/a.md");
            private_commit(s, root);
            dupe_succeeds(s, root, &["push", "-q", "origin", "main"]);
            let tip = commit_of(s, root, "main");
            graft.graft(s, root, &tip, &[]);
            assert!(graft.unpushed(s, root, "old", false), "{graft:?}");
            assert!(!graft.unpushed(s, root, "old", true), "{graft:?}");
            clean(s, root);
            detaches_keeping_every_file(s, root, &["detach"]);
        }
    });
}

/// The commit `name` names in the private repository at `root`, by its full hash.
fn commit_of(s: &Scenario, root: &Path, name: &str) -> String {
    let output = s
        .private(root)
        .git(["rev-parse", "--verify", name])
        .succeeds();
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

/// `git dupe <words>` from `root` succeeds and leaves it unattached, every file of the
/// working tree as it was.
fn detaches_keeping_every_file(s: &Scenario, root: &Path, words: &[&str]) {
    let before = Tree::working(root);
    dupe_succeeds(s, root, words);
    detached(root);
    let changed = before.changed_in(&Tree::working(root));
    assert!(changed.is_empty(), "changed: {changed:?}");
}

#[test]
fn unreadable_missing_head_and_empty_private_directory() {
    under_each_release(|s| {
        for cause in ["missing-head", "empty-directory"] {
            let transfer = s.pushed_workspace(cause);
            stale_timestamp(&transfer.root);
            let private = transfer.root.join(".git/dupe");
            if cause == "missing-head" {
                fs::remove_file(private.join("HEAD")).unwrap();
            } else {
                fs::remove_dir_all(&private).unwrap();
                fs::create_dir(&private).unwrap();
            }
            let before = transfer.everything();
            let output = s.git(["dupe", "detach"]).from(&transfer.root).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let lines: Vec<_> = output
                .stderr
                .split_inclusive(|&byte| byte == b'\n')
                .collect();
            assert!(lines.len() > 1, "{output:?}");
            let last = lines.last().unwrap().strip_prefix(b"fatal: ").unwrap();
            names(last, b"git dupe init");
            names(last, b"git dupe detach --force");
            assert!(output.lines("warning").is_empty(), "{output:?}");
            before.unchanged();
            assert!(private.is_dir());
            assert!(region(&transfer.root).is_some());
        }
    });
}

#[test]
fn untracked_files_do_not_refuse_even_when_status_config_lists_them() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("untracked");
        s.private(&transfer.root)
            .git(["config", "status.showUntrackedFiles", "all"])
            .succeeds();
        write(&transfer.root, "notes/untracked.md", b"untracked\n");
        s.git(["dupe", "detach"]).from(&transfer.root).succeeds();
        assert!(!transfer.root.join(".git/dupe").exists());
    });
}

#[test]
fn unborn_branch_is_safe_until_hide_stages_gitdupe() {
    under_each_release(|s| {
        let empty = s.dir().join("empty");
        s.attached_project(&empty);
        s.git(["dupe", "detach"]).from(&empty).succeeds();
        assert!(!empty.join(".git/dupe").exists());

        let staged = s.dir().join("staged");
        s.attached_project(&staged);
        fs::create_dir(staged.join("notes")).unwrap();
        s.git(["dupe", "hide", "notes"]).from(&staged).succeeds();
        stale_file_timestamp(&staged.join(".gitdupe"));
        let before = Tree::of(&staged);
        let output = s.git(["dupe", "detach"]).from(&staged).run();
        names(
            refusal(&output, &[b"git dupe detach --force"]),
            b"not committed",
        );
        unchanged(&before, &staged);
        assert!(staged.join(".git/dupe").is_dir());
        assert!(region(&staged).is_some());
    });
}

#[test]
fn stale_region_is_not_settled_on_refusal() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("stale");
        let root = &transfer.root;
        let exclude = root.join(".git/info/exclude");
        let region_before = fs::read(&exclude).unwrap();
        write(root, ".gitdupe", b"notes\n.vscode\nextra\n");
        write(root, "notes/a.md", b"uncommitted\n");
        let before = before_refusal(&transfer);
        let output = s.git(["dupe", "detach"]).from(root).run();
        names(
            refusal(&output, &[b"git dupe detach --force"]),
            b"not committed",
        );
        assert!(output.lines("warning").is_empty(), "{output:?}");
        assert_eq!(fs::read(&exclude).unwrap(), region_before);
        before.unchanged();
        assert!(root.join(".git/dupe").is_dir());
        assert!(region(root).is_some());
    });
}

#[test]
fn info_link_with_region_refuses_even_force_before_private_runs() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("linked-info");
        let root = &transfer.root;
        let info = root.join(".git/info");
        let target = s.dir().join("info-elsewhere");
        fs::rename(&info, &target).unwrap();
        symlink(&target, &info).unwrap();
        let before = before_refusal(&transfer);
        // Everything records the link; explicitly snapshot the directory beyond it too.
        let beyond = Tree::of(&target);
        for words in [
            ["dupe", "detach"].as_slice(),
            &["dupe", "detach", "--force"],
        ] {
            let (output, runs) = run_traced(s.git(words).from(root), &s.dir().join("trace"));
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            names_path(output.only_line("fatal"), as_git_spells(&info).as_bytes());
            let own = runs.own();
            assert_eq!(own.commands(), [b"rev-parse".as_slice()], "{own:?}");
            before.unchanged();
            unchanged(&beyond, &target);
            assert!(root.join(".git/dupe").is_dir());
            assert!(region(root).is_some());
        }
    });
}

#[test]
fn info_link_without_region_allows_detach_and_preserves_target() {
    under_each_release(|s| {
        let root = s.dir().join("no-region");
        s.unattached_project(&root);
        let info = root.join(".git/info");
        let target = s.dir().join("info-without-region");
        fs::rename(&info, &target).unwrap();
        write(&target, "exclude", b"# user excludes\n*.local\n");
        symlink(&target, &info).unwrap();
        s.init(&root);
        assert!(region(&root).is_none());
        let link_before = fs::read_link(&info).unwrap();
        let beyond = Tree::of(&target);
        let output = s.git(["dupe", "detach"]).from(&root).succeeds();
        assert!(!root.join(".git/dupe").exists());
        assert_eq!(fs::read_link(&info).unwrap(), link_before);
        unchanged(&beyond, &target);
        assert!(
            !output
                .lines("warning")
                .iter()
                .any(|line| holds(line, b"region cannot be maintained")),
            "{output:?}"
        );
    });
}
