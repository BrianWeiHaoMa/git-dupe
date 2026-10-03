//! `git dupe clone` on a second machine (G2, F2, `In use` 5, the product's `Done when`):
//! the private files arrive, the second machine's own files stay, `.gitdupe` hides there
//! what it hides on the first, and `pull` and `push` move private work both ways.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, leaving_public_git, names, region_rules, under_each_release, write,
};

/// The privately tracked files of `daily_state`, the first machine's private history.
const PRIVATE: [&str; 5] = [
    ".gitdupe",
    "notes/a.md",
    ".env.local",
    ".vscode/settings.json",
    "docs/notes.md",
];

/// The value of `name` in the private local configuration of the workspace at `dir`.
fn key(s: &Scenario, dir: &Path, name: &str, expected: &str) {
    let output = s
        .private(dir)
        .git(["config", "--local", "--get-all", name])
        .run();
    assert_eq!(output.end, End::Code(0), "{name}: {output:?}");
    assert_eq!(output.stdout, format!("{expected}\n").as_bytes(), "{name}");
}

/// The commit the private `HEAD` of the workspace at `dir` names.
fn private_head(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.private(dir).git(["rev-parse", "HEAD"]).succeeds().stdout
}

/// `git dupe <words>` from `dir`, with an identity for a commit.
fn dupe(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let identity = [
        "-c",
        "user.name=Scenario",
        "-c",
        "user.email=scenario@example.invalid",
        "dupe",
    ];
    s.git(identity.iter().chain(words)).from(dir).run()
}

#[test]
fn a_second_machine_attaches_and_keeps_its_own_differing_env_local() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        for (name, value) in [
            ("user.name", "Second Name"),
            ("user.email", "second@example.invalid"),
        ] {
            s.git(["config", "--local", name, value])
                .from(second)
                .succeeds();
        }
        write(second, ".env.local", b"mine\n");
        let before = Tree::working(second);
        let url = m.first.private_remote.as_os_str();

        let cloned = leaving_public_git(
            second,
            s.git([OsStr::new("dupe"), OsStr::new("clone"), url])
                .from(second),
        );
        assert_eq!(cloned.end, End::Code(0), "{cloned:?}");
        for level in ["fatal", "error", "hint"] {
            assert!(cloned.lines(level).is_empty(), "{level}: {cloned:?}");
        }
        // The one file kept that differs is named, and nothing else.
        let warnings = cloned.lines("warning");
        assert_eq!(warnings.len(), 1, "{cloned:?}");
        names(warnings[0], b".env.local");
        names(warnings[0], b"git dupe restore -- .env.local");

        // The second machine's file is kept; every other private file arrives as
        // committed; every entry that was there is as it was.
        assert_eq!(fs::read(second.join(".env.local")).unwrap(), b"mine\n");
        for path in PRIVATE.iter().filter(|path| **path != ".env.local") {
            assert_eq!(
                fs::read(second.join(path)).unwrap(),
                fs::read(first.join(path)).unwrap(),
                "{path}"
            );
        }
        let mut changed = before.changed_in(&Tree::working(second));
        changed.sort();
        let mut arrived: Vec<_> = [
            ".gitdupe",
            "notes",
            "notes/a.md",
            ".vscode",
            ".vscode/settings.json",
            "docs/notes.md",
        ]
        .iter()
        .map(|path| second.join(path))
        .collect();
        arrived.sort();
        assert_eq!(changed, arrived);

        // Shown modified, and nothing else changed.
        let status = s
            .git(["dupe", "status", "--porcelain", "-z"])
            .from(second)
            .succeeds();
        assert_eq!(status.stdout, b" M .env.local\0", "{status:?}");

        // `.gitdupe` arrived: the region is the first machine's, and a new file under a
        // hidden directory is invisible to the project's Git.
        assert_eq!(region_rules(second), region_rules(first));
        write(second, "notes/x.md", b"new\n");
        let public = s
            .git(["status", "--porcelain", "--untracked-files=all"])
            .from(second)
            .succeeds();
        assert!(public.stdout.is_empty(), "{public:?}");

        // The private repository is configured as F2 says, `origin` as typed, the branch
        // tracking it, the identity of the second machine's own local configuration.
        key(s, second, "status.showUntrackedFiles", "no");
        key(s, second, "advice.statusHints", "false");
        key(s, second, "core.worktree", "../..");
        key(s, second, "user.name", "Second Name");
        key(s, second, "user.email", "second@example.invalid");
        key(s, second, "remote.origin.url", url.to_str().unwrap());
        key(s, second, "branch.main.remote", "origin");
        key(s, second, "branch.main.merge", "refs/heads/main");
        let head = s.private(second).git(["symbolic-ref", "HEAD"]).succeeds();
        assert_eq!(head.stdout, b"refs/heads/main\n");
        assert_eq!(private_head(s, second), private_head(s, first));
        // The private index is the branch's tree: nothing staged.
        let staged = s.private(second).git(["diff", "--cached", "--quiet"]).run();
        assert_eq!(staged.end, End::Code(0), "{staged:?}");

        // Standard output is Git's `init` line and nothing else: what Git's own `init`
        // prints for the same private Git directory.
        let detached = s.git(["dupe", "detach", "--force"]).from(second).run();
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        let gits = s
            .git(["init", "--initial-branch=main"])
            .from(second)
            .variable("GIT_DIR", second.join(".git/dupe"))
            .variable("GIT_WORK_TREE", second)
            .succeeds();
        assert_eq!(cloned.stdout, gits.stdout, "{cloned:?}");
    });
}

#[test]
fn pull_and_push_move_private_commits_both_ways_between_the_machines() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        write(second, ".env.local", b"mine\n");
        let url = m.first.private_remote.to_str().unwrap();
        let cloned = s.git(["dupe", "clone", url]).from(second).run();
        assert_eq!(cloned.end, End::Code(0), "{cloned:?}");

        // From the second machine to the first: its kept `.env.local`, committed.
        for words in [
            &["commit", "-qam", "the second machine's settings"][..],
            &["push"],
        ] {
            let output = dupe(s, second, words);
            assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
        }
        let pulled = dupe(s, first, &["pull"]);
        assert_eq!(pulled.end, End::Code(0), "{pulled:?}");
        assert_eq!(private_head(s, first), private_head(s, second));
        assert_eq!(fs::read(first.join(".env.local")).unwrap(), b"mine\n");

        // From the first machine to the second: a new private note.
        write(first, "notes/b.md", b"from the first machine\n");
        for words in [
            &["add", "notes/b.md"][..],
            &["commit", "-qm", "a note"],
            &["push"],
        ] {
            let output = dupe(s, first, words);
            assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
        }
        let pulled = dupe(s, second, &["pull"]);
        assert_eq!(pulled.end, End::Code(0), "{pulled:?}");
        assert_eq!(private_head(s, second), private_head(s, first));
        assert_eq!(
            fs::read(second.join("notes/b.md")).unwrap(),
            b"from the first machine\n"
        );
        let public = s
            .git(["status", "--porcelain", "--untracked-files=all"])
            .from(second)
            .succeeds();
        assert!(public.stdout.is_empty(), "{public:?}");
    });
}

#[test]
fn clone_from_a_subdirectory_writes_what_it_writes_from_the_root() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let third = s.dir().join("project-third");
        s.public_clone(&m.first.root, &third);
        let below = third.join("docs");
        let url = m.first.private_remote.to_str().unwrap();
        for (root, from) in [(&m.root, &m.root), (&third, &below)] {
            write(root, ".env.local", b"mine\n");
            let cloned = s.git(["dupe", "clone", url]).from(from).run();
            assert_eq!(cloned.end, End::Code(0), "{cloned:?}");
            // The kept file is named by its path from the root, where its command runs.
            let warnings = cloned.lines("warning");
            assert_eq!(warnings.len(), 1, "{cloned:?}");
            names(
                warnings[0],
                b"'git dupe restore -- .env.local' run from the root replaces it",
            );
            assert!(root.join("notes/a.md").is_file());
        }
        let git = Path::new(".git");
        let from_the_root = Tree::relative(&m.root).without(&[git]);
        let from_below = Tree::relative(&third).without(&[git]);
        let changed = from_the_root.changed_in(&from_below);
        assert!(changed.is_empty(), "changed: {changed:?}");
        let listed = |root: &Path| {
            s.private(root)
                .git(["ls-files", "-z", "--stage"])
                .succeeds()
                .stdout
        };
        assert_eq!(listed(&m.root), listed(&third));
        assert_eq!(region_rules(&m.root), region_rules(&third));

        // What the warning names, run where it says, replaces the kept file.
        s.git(["dupe", "restore", "--", ".env.local"])
            .from(&third)
            .succeeds();
        assert_eq!(
            fs::read(third.join(".env.local")).unwrap(),
            fs::read(m.first.root.join(".env.local")).unwrap()
        );
    });
}
