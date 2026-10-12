use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use super::super::output::End;
use super::*;

/// An attached project in the scenario's directory, and its common Git directory.
fn attached(s: &Scenario) -> (PathBuf, PathBuf) {
    let dir = s.dir().join("project");
    s.attached_project(&dir);
    let common = dir.join(".git");
    (dir, common)
}

/// What a failed check said.
fn said(failed: Box<dyn Any + Send>) -> String {
    match failed.downcast::<String>() {
        Ok(message) => *message,
        Err(failed) => failed
            .downcast_ref::<&str>()
            .map(|message| message.to_string())
            .unwrap_or_default(),
    }
}

/// Waits, `LIMIT` at most, until the log of `command` holds an event `found` accepts,
/// and returns it.
fn logged(command: &Watched, found: impl Fn(&Event) -> bool) -> Event {
    let deadline = Instant::now() + LIMIT;
    loop {
        if let Some(event) = command.runs.events().into_iter().find(&found) {
            return event;
        }
        assert!(
            Instant::now() < deadline,
            "{}: {}",
            command.named,
            command.runs.log()
        );
        thread::sleep(Duration::from_millis(5));
    }
}

/// A run started while another descriptor holds the lock is found holding it, and the
/// command's check fails naming that run: the harness's own hold, taken outside the
/// watch, stands for git-dupe's, which is how a git-dupe starting a run under its lock
/// is caught (R10).
#[test]
fn a_run_started_while_the_lock_is_held_fails_the_check_naming_it() {
    let s = Scenario::of_the_first_release();
    let (dir, common) = attached(&s);
    let watch = s.watching_the_lock("watch", &common);
    let held = hold_the_lock(&common);
    let status = watch.start(&dir, &["status"]);
    // The first run has its verdict before its answer, so once the second has started.
    logged(&status, |event| {
        matches!(event, Event::Started { run: 2, .. })
    });
    drop(held);
    let Err(failed) = catch_unwind(AssertUnwindSafe(|| status.wait())) else {
        panic!("a run started while the lock was held passed the check");
    };
    let said = said(failed);
    assert!(
        said.contains("started Git while it held the lock") && said.contains("rev-parse"),
        "{said}"
    );
}

/// A run never answered gives up within its bound, and the check fails naming it
/// instead of hanging; the answers an earlier command of the same watch was given
/// answer none of a later command's runs, each command having a control directory of
/// its own.
#[test]
fn a_run_left_unanswered_gives_up_and_fails_the_check_naming_it() {
    let s = Scenario::of_the_first_release();
    let (dir, common) = attached(&s);
    let watch = s.watching_the_lock("watch", &common);
    let answered = watch.run(&dir, &["status"]);
    assert_eq!(answered.end, End::Code(0), "{answered:?}");
    watch.withholding_answers(100);
    let began = Instant::now();
    let Err(failed) = catch_unwind(AssertUnwindSafe(|| watch.run(&dir, &["status"]))) else {
        panic!("a command whose runs had no answer passed the check");
    };
    let said = said(failed);
    assert!(
        said.contains("had no answer within its bound") && said.contains("rev-parse"),
        "{said}"
    );
    assert!(began.elapsed() < LIMIT, "it took {:?}", began.elapsed());
}

/// Two commands of one watch running at once, with no hold of the scenario's, may each
/// hold the lock in its replacement as the other starts a run: neither is blamed for
/// the other's hold, however long the replacements take.
#[test]
fn two_commands_at_once_are_not_blamed_for_each_others_lock() {
    let s = Scenario::of_the_first_release();
    let (dir, common) = attached(&s);
    let other = s.dir().join("other");
    s.linked_worktree(&dir, &other);
    s.init(&other);
    // A long user's part makes each replacement long enough to overlap the other's runs.
    let exclude = common.join("info/exclude");
    let comment: Vec<u8> = (0..300_000)
        .flat_map(|n| format!("# the user's line {n}\n").into_bytes())
        .collect();
    fs::write(&exclude, [comment, fs::read(&exclude).unwrap()].concat()).unwrap();
    let watch = s.watching_the_lock("watch", &common);
    for n in 0..5 {
        let (main, linked) = (format!("main-{n}"), format!("linked-{n}"));
        let hiding = [
            watch.start(&dir, &["hide", &main]),
            watch.start(&other, &["hide", &linked]),
        ];
        for hidden in hiding.map(Watched::wait) {
            assert_eq!(hidden.end, End::Code(0), "{hidden:?}");
        }
    }
}

/// A watched command dropped before it was waited for, as a failing scenario drops it,
/// leaves no script waiting for an answer.
#[test]
fn a_watched_command_dropped_unwaited_leaves_no_script_waiting() {
    let s = Scenario::of_the_first_release();
    let (dir, common) = attached(&s);
    let watch = s.watching_the_lock("watch", &common);
    watch.withholding_answers(LOOKS);
    let status = watch.start(&dir, &["status"]);
    let Event::Started { process, .. } =
        logged(&status, |event| matches!(event, Event::Started { .. }))
    else {
        unreachable!("a start was looked for");
    };
    let began = Instant::now();
    drop(status);
    while !ended(&process) {
        assert!(
            began.elapsed() < LIMIT,
            "the script waiting for its answer still runs"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
