//! A linked worktree's commands run Git as the main worktree's do: the same runs in the
//! same order, none following either repository's file count, because its private
//! repository and its working tree are found from the one locate run and recorded
//! without a Git run of their own (G22, R4, `Holds/G1`). `init` sets the keys only where
//! they are unfinished: after `git worktree move`, and not again once finished.

use std::ffi::OsStr;

use crate::harness::{End, Runs, Scenario, Worktree, run_traced, under_each_release, write};

const IDENTITY: [&str; 6] = [
    "-c",
    "maintenance.auto=false",
    "-c",
    "user.name=Scenario",
    "-c",
    "user.email=scenario@example.invalid",
];

/// The command words of the runs git-dupe itself started.
fn commands(runs: &Runs) -> Vec<Vec<u8>> {
    runs.own()
        .commands()
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

/// Whether a run git-dupe started set a key with `--replace-all`.
fn sets_keys(runs: &Runs) -> bool {
    runs.own()
        .words()
        .iter()
        .any(|words| words.iter().any(|word| word == b"--replace-all"))
}

/// A project of `files` public files under `name`, and the worktree commands run in: its
/// main worktree, or a linked one beside it.
fn worktree(s: &Scenario, name: &str, linked: bool, files: usize) -> Worktree {
    let project = s.dir().join(name);
    s.unattached_project(&project);
    for index in 0..files {
        write(&project, &format!("src/file-{index}"), b"public\n");
    }
    s.git(["add", "src"]).from(&project).succeeds();
    s.commit_public(&project);
    if !linked {
        return Worktree::read(s, &project);
    }
    let root = s.dir().join(format!("{name}-linked"));
    s.linked_worktree(&project, &root);
    Worktree::read(s, &root)
}

/// The runs of each command of a day in `wt`, with `files` private files: `init`, `init`
/// again, `add -A`, `status`, `hide`, `commit`, `detach --force`, and `clone` of the
/// history it pushed. Checks that only the first `init` sets keys.
fn day(s: &Scenario, wt: &Worktree, name: &str, files: usize) -> Vec<Vec<Vec<u8>>> {
    let root = &wt.root;
    let trace = s.dir().join(format!("{name}-trace"));
    let remote = s.dir().join(format!("{name}-private.git"));
    s.bare_repository(&remote);
    let traced = |words: &[&str]| {
        let (output, runs) = run_traced(s.git(words).from(root), &trace);
        assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
        runs
    };

    let first = traced(&["dupe", "init"]);
    assert!(sets_keys(&first));
    let again = traced(&["dupe", "init"]);
    assert!(!sets_keys(&again));
    write(root, ".gitdupe", b"notes\n");
    for index in 0..files {
        write(root, &format!("notes/file-{index}"), b"private\n");
    }
    let add = traced(&["dupe", "add", "-A"]);
    let listed = wt.private(s).git(["ls-files", "-z"]).succeeds();
    assert_eq!(
        listed.stdout.iter().filter(|&&byte| byte == 0).count(),
        files + 1
    );
    let status = traced(&["dupe", "status"]);
    let hide = traced(&["dupe", "hide", "scratch"]);
    let commit = traced(
        &IDENTITY
            .iter()
            .copied()
            .chain(["dupe", "commit", "-qm", "day"])
            .collect::<Vec<_>>(),
    );
    s.git([
        OsStr::new("dupe"),
        OsStr::new("remote"),
        OsStr::new("add"),
        OsStr::new("origin"),
        remote.as_os_str(),
    ])
    .from(root)
    .succeeds();
    s.git(["dupe", "push", "-q", "origin", "HEAD:refs/heads/main"])
        .from(root)
        .succeeds();
    let detach = traced(&["dupe", "detach", "--force"]);
    let clone = traced(&[
        "dupe",
        "clone",
        remote.to_str().expect("a UTF-8 scenario directory"),
        "-b",
        "main",
    ]);
    assert!(wt.private_directory().join("HEAD").is_file());
    [first, again, add, status, hide, commit, detach, clone]
        .iter()
        .map(commands)
        .collect()
}

#[test]
fn a_linked_worktree_runs_git_as_the_main_one_does_whatever_either_repository_holds() {
    under_each_release(|s| {
        let main = worktree(s, "main", false, 5);
        let few = worktree(s, "few", true, 5);
        let many = worktree(s, "many", true, 500);
        let main_day = day(s, &main, "main", 5);
        let few_day = day(s, &few, "few", 5);
        let many_day = day(s, &many, "many", 500);
        assert_eq!(few_day, main_day);
        assert_eq!(many_day, few_day);
    });
}

#[test]
fn after_a_move_init_sets_the_keys_once_and_runs_no_git_init() {
    under_each_release(|s| {
        let wt = worktree(s, "project", true, 5);
        s.init(&wt.root);
        let moved = s.dir().join("moved");
        s.git([
            OsStr::new("worktree"),
            OsStr::new("move"),
            wt.root.as_os_str(),
            moved.as_os_str(),
        ])
        .from(&s.dir().join("project"))
        .succeeds();
        let trace = s.dir().join("trace");
        let traced = || {
            let (output, runs) = run_traced(s.git(["dupe", "init"]).from(&moved), &trace);
            assert_eq!(output.end, End::Code(0), "{output:?}");
            runs
        };
        let repaired = traced();
        assert!(sets_keys(&repaired));
        assert_eq!(repaired.own().of("init"), 0);
        let finished = traced();
        assert!(!sets_keys(&finished));
        assert_eq!(finished.own().of("init"), 0);
        // Repairing adds the key runs alone to what a finished `init` runs.
        let mut without_keys = commands(&repaired);
        let keys = repaired
            .own()
            .words()
            .iter()
            .filter(|words| words.iter().any(|word| word == b"--replace-all"))
            .count();
        assert!(keys > 0);
        let finished_commands = commands(&finished);
        let configs = |sequence: &[Vec<u8>]| {
            sequence
                .iter()
                .filter(|command| command.as_slice() == b"config")
                .count()
        };
        assert_eq!(
            configs(&without_keys),
            configs(&finished_commands) + keys + 2
        );
        without_keys.retain(|command| command.as_slice() != b"config");
        let mut finished_without = finished_commands.clone();
        finished_without.retain(|command| command.as_slice() != b"config");
        assert_eq!(without_keys, finished_without);
    });
}
