//! `init` and `clone` name a Git on `PATH` older than F4's range in one `warning:`, before
//! any run such a Git can fail, and go on; a supported Git, one past the range, and an
//! answer that names no release get no line (F4, E5). The older Git is the stand-in `git`
//! of the kill harness answering `git version` as such a release does, every other run
//! being the release's own. Only `init` and `clone` ask, once each (G22).

use std::path::Path;

use crate::harness::{
    End, ForwardEffect, Output, Scenario, daily_edited, names, run_traced, under_each_release,
    write,
};

/// The range the line names, as `git dupe help` names it.
const RANGE: &[u8] = b"Git 2.43.0 or newer, below 3.0.0";

/// `git dupe <words>` from `dir` beside a `git` whose `version` answers `answer`.
fn beside(s: &Scenario, control: &str, dir: &Path, words: &[&str], answer: &[u8]) -> Output {
    s.forwarded(
        &format!("control-{control}"),
        words,
        false,
        &["version"],
        ForwardEffect::Print(answer.to_vec()),
    )
    .run(dir)
}

/// Standard error opens with the one line naming `spelled` and the range, and no other line
/// of the run names the Git on `PATH`.
fn named_first(output: &Output, spelled: &[u8]) {
    let first = output
        .stderr
        .split(|&byte| byte == b'\n')
        .next()
        .and_then(|line| line.strip_prefix(b"warning: "))
        .unwrap_or_else(|| panic!("standard error opens with no warning: {output:?}"));
    names(
        first,
        &[&b"the git on PATH is version "[..], spelled, b","].concat(),
    );
    names(first, RANGE);
    assert_eq!(unnamed_count(output), 1, "{output:?}");
}

/// How many lines of the run name the Git on `PATH`.
fn unnamed_count(output: &Output) -> usize {
    output
        .stderr
        .split(|&byte| byte == b'\n')
        .filter(|line| line.windows(15).any(|part| part == b"the git on PATH"))
        .count()
}

#[test]
fn init_and_clone_name_an_older_git_first_and_go_on() {
    under_each_release(|s| {
        for (index, (answer, spelled)) in [
            (&b"git version 2.34.1\n"[..], &b"2.34.1"[..]),
            (
                b"git version 2.42.4 (Apple Git-150)\n",
                b"2.42.4 (Apple Git-150)",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let dir = s.dir().join(format!("project-{index}"));
            s.unattached_project(&dir);
            let init = beside(s, &format!("init-{index}"), &dir, &["init"], answer);
            assert_eq!(init.end, End::Code(0), "{init:?}");
            named_first(&init, spelled);
            assert!(dir.join(".git/dupe").is_dir(), "{init:?}");

            // Before the runs of the public places, whatever follows: a refusal of G18, and
            // a clone that goes on to its fetch and its warning of a kept file.
            let m = s.second_machine(&format!("machine-{index}"));
            write(&m.root, ".env.local", b"mine\n");
            let public = m.first.root.to_str().unwrap();
            let refused = beside(
                s,
                &format!("refused-{index}"),
                &m.root,
                &["clone", public],
                answer,
            );
            assert_eq!(refused.end, End::Code(128), "{refused:?}");
            named_first(&refused, spelled);
            assert_eq!(refused.lines("fatal").len(), 1, "{refused:?}");
            assert!(!m.root.join(".git/dupe").exists(), "{refused:?}");

            let url = m.first.private_remote.to_str().unwrap();
            let clone = beside(
                s,
                &format!("clone-{index}"),
                &m.root,
                &["clone", url],
                answer,
            );
            assert_eq!(clone.end, End::Code(0), "{clone:?}");
            named_first(&clone, spelled);
            let warnings = clone.lines("warning");
            assert_eq!(warnings.len(), 2, "{clone:?}");
            names(warnings[1], b".env.local");
            assert!(m.root.join("notes/a.md").is_file(), "{clone:?}");

            // A refusal of G4 too comes after it.
            let tracked = s.dir().join(format!("tracked-{index}"));
            s.unattached_project(&tracked);
            write(&tracked, ".gitdupe", b"notes\n");
            s.git(["add", ".gitdupe"]).from(&tracked).succeeds();
            s.commit_public(&tracked);
            let refused = beside(s, &format!("tracked-{index}"), &tracked, &["init"], answer);
            assert_eq!(refused.end, End::Code(128), "{refused:?}");
            named_first(&refused, spelled);
            assert!(!tracked.join(".git/dupe").exists(), "{refused:?}");
        }
    });
}

#[test]
fn a_supported_git_one_past_the_range_and_an_unreadable_answer_get_no_line() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.unattached_project(&dir);
        let init = s.git(["dupe", "init"]).from(&dir).run();
        assert_eq!(init.end, End::Code(0), "{init:?}");
        assert!(init.stderr.is_empty(), "{init:?}");

        let m = s.second_machine("machine");
        let url = m.first.private_remote.to_str().unwrap();
        let clone = s.git(["dupe", "clone", url]).from(&m.root).run();
        assert_eq!(clone.end, End::Code(0), "{clone:?}");
        assert_eq!(unnamed_count(&clone), 0, "{clone:?}");
        assert!(clone.lines("warning").is_empty(), "{clone:?}");

        for (index, answer) in [
            &b"git version 3.0.0\n"[..],
            b"git version 2.55.0.windows.5\n",
            b"git version 2.43.0.rc1\n",
            b"not a version\n",
            b"",
        ]
        .into_iter()
        .enumerate()
        {
            let dir = s.dir().join(format!("other-{index}"));
            s.unattached_project(&dir);
            let init = beside(s, &format!("other-{index}"), &dir, &["init"], answer);
            assert_eq!(init.end, End::Code(0), "{answer:?}: {init:?}");
            assert!(init.stderr.is_empty(), "{answer:?}: {init:?}");
            assert!(dir.join(".git/dupe").is_dir(), "{answer:?}: {init:?}");
        }
    });
}

/// `init` and `clone` ask right after the locate run and once; the commands of a day's
/// work, `Done when`'s counted six among them, never ask.
#[test]
fn only_init_and_clone_ask_and_each_once_before_anything_else() {
    under_each_release(|s| {
        let trace = s.dir().join("trace");
        let dir = s.dir().join("project");
        s.unattached_project(&dir);
        let (init, runs) = run_traced(s.git(["dupe", "init"]).from(&dir), &trace);
        assert_eq!(init.end, End::Code(0), "{init:?}");
        let own = runs.own();
        let commands = own.commands();
        assert_eq!(commands[..2], [&b"rev-parse"[..], b"version"], "{own:?}");
        assert_eq!(
            commands.iter().filter(|word| **word == b"version").count(),
            1,
            "{own:?}"
        );

        let dir = daily_edited(s, "daily");
        let identity = [
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
        ];
        for words in [
            &["status"][..],
            &["add", "."],
            &["commit", "-qm", "x"],
            &["switch", "-qc", "other"],
            &["log", "-1"],
            &["clean", "-n"],
            &["hide", "scratch"],
            &["unhide", "scratch"],
        ] {
            let words: Vec<&str> = identity
                .iter()
                .chain(&["dupe"])
                .chain(words)
                .copied()
                .collect();
            let (output, runs) = run_traced(s.git(&words).from(&dir), &trace);
            assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
            let own = runs.own();
            assert!(
                !own.commands().contains(&&b"version"[..]),
                "{words:?}: {own:?}"
            );
        }
    });
}
