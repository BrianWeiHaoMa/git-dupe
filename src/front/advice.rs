//! The hint after a word passed through whose outcome Git's advice names a plain `git`
//! command for (`Holds/G19`, G25): `rebase` and `am` stopped, `cherry-pick` and `revert`
//! stopped, `checkout` and `switch` leaving a branch for a detached `HEAD`, and `branch
//! -d` refusing. Copied under `git dupe`, Git's command would act on the project's
//! repository; the hint names the `git dupe` form.
//!
//! What the code below cannot show:
//!
//! - Git's own output reaches the user untouched and is never read (R5, `Composition/Git`):
//!   each hint is decided from the run's end and the private repository's state, and
//!   written after every byte of Git's, before settle's warnings. It follows the state,
//!   not Git's `advice.*` settings, so it still follows where a setting silences Git's
//!   advice. The exit status is the run's (F9).
//! - The words read are the run's as typed, or those an alias chain reached (`alias`).
//!   They are matched as exact words: a bundle or an abbreviation of `--detach`, `-q`,
//!   `-d`, or `--delete` is not read, so a hint may follow where Git printed no advice,
//!   never one the state makes false. None follows a run with `-h` or `--help` before the
//!   first `--` of the words read, which an expansion's own `--` can move a typed `-h`
//!   after, nor a run that did not start, nor one against a Git directory where no private
//!   repository stands, as where a help request runs in a worktree no `init` attached.
//! - "Did not exit 0" is any end but exit 0, a signal included: an interrupted rebase is a
//!   stopped one. An end named by its number, 0 or 1, is never a signal's.
//! - The runs a family adds are fixed for its words (G22, R4): `checkout` and `switch` ask
//!   the private `symbolic-ref -q HEAD` before the run and, after one that exited 0, again,
//!   both only where the words admit the hint; `cherry-pick` and `revert` ask `rev-parse
//!   -q --verify` of their pseudo-reference after a run that did not exit 0; `rebase`,
//!   `am`, and `branch` add none. A question that answers with any status but its two
//!   gives no hint and leaves the command's status as it was. The `symbolic-ref` before the
//!   run decides nothing about the run (N8).
//! - `CHERRY_PICK_HEAD` and `REVERT_HEAD` are asked of Git, because they live in the
//!   reference store, which may hold no file for them (S15); `rebase-merge` and
//!   `rebase-apply` are directories in every store, read by `lstat` (S12).
//! - The `branch -d` hint follows exit 1 alone, so a branch that does not exist draws it
//!   too; it says what deletes a branch `-d` keeps, which is true either way.

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use super::lines::{self, Level};
use super::outcome::Outcome;
use crate::runner::{Against, End, Run};

const REBASE: &[u8] = b"a private rebase that stopped goes on with 'git dupe rebase --continue' \
    once 'git dupe add <path>' has marked each conflict resolved, skips the commit with \
    'git dupe rebase --skip', or ends with 'git dupe rebase --abort'; plain 'git add' and \
    'git rebase' act on the project's repository";

const AM: &[u8] = b"a private 'am' that stopped goes on with 'git dupe am --continue' once \
    'git dupe add <path>' has marked each conflict resolved, skips the patch with \
    'git dupe am --skip', or ends with 'git dupe am --abort'; plain 'git add' and 'git am' \
    act on the project's repository";

const CHERRY_PICK: &[u8] = b"a private cherry-pick that stopped goes on with \
    'git dupe cherry-pick --continue' once 'git dupe add <path>' has marked each conflict \
    resolved, or, where the pick is empty, with 'git dupe commit --allow-empty'; it skips \
    the commit with 'git dupe cherry-pick --skip', or ends with \
    'git dupe cherry-pick --abort'; plain 'git add', 'git commit', and 'git cherry-pick' act \
    on the project's repository";

const REVERT: &[u8] = b"a private revert that stopped goes on with 'git dupe revert --continue' \
    once 'git dupe add <path>' has marked each conflict resolved, skips the commit with \
    'git dupe revert --skip', or ends with 'git dupe revert --abort'; plain 'git add' and \
    'git revert' act on the project's repository";

const DETACHED: &[u8] = b"HEAD is now detached in the private repository: \
    'git dupe switch -c <new-branch-name>' keeps commits made here on a new branch, and \
    'git dupe switch -' goes back; plain 'git switch' acts on the project's repository";

const UNMERGED: &[u8] = b"'git dupe branch -D <branch>' deletes a private branch that \
    'git dupe branch -d' keeps because it is not fully merged; plain 'git branch -D' acts on \
    the project's repository";

/// The families, one per command word.
const FAMILIES: [Family; 7] = [
    Family {
        word: b"rebase",
        excluding: &[],
        requiring: &[],
        trigger: Trigger::Stopped,
        line: REBASE,
    },
    Family {
        word: b"am",
        excluding: &[],
        requiring: &[],
        trigger: Trigger::Stopped,
        line: AM,
    },
    Family {
        word: b"cherry-pick",
        excluding: &[],
        requiring: &[],
        trigger: Trigger::Picking("CHERRY_PICK_HEAD"),
        line: CHERRY_PICK,
    },
    Family {
        word: b"revert",
        excluding: &[],
        requiring: &[],
        trigger: Trigger::Picking("REVERT_HEAD"),
        line: REVERT,
    },
    Family {
        word: b"checkout",
        excluding: &[b"--detach", b"-q", b"--quiet"],
        requiring: &[],
        trigger: Trigger::Detached,
        line: DETACHED,
    },
    Family {
        word: b"switch",
        excluding: &[b"--detach", b"-d", b"-q", b"--quiet"],
        requiring: &[],
        trigger: Trigger::Detached,
        line: DETACHED,
    },
    Family {
        word: b"branch",
        excluding: &[],
        requiring: &[b"-d", b"--delete"],
        trigger: Trigger::ExitedOne,
        line: UNMERGED,
    },
];

/// A command word whose run Git's advice may follow, and when the hint follows it.
struct Family {
    word: &'static [u8],
    /// Words before the first `--` any of which keeps the hint away: Git prints no advice
    /// after them.
    excluding: &'static [&'static [u8]],
    /// Words one of which must stand among the words, when there are any.
    requiring: &'static [&'static [u8]],
    trigger: Trigger,
    line: &'static [u8],
}

/// The run's end and the state after which a family's hint follows (S12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    /// A run that did not exit 0, while `rebase-merge` or `rebase-apply` is a directory in
    /// the private Git directory.
    Stopped,
    /// A run that did not exit 0, while `rev-parse -q --verify` of this name exits 0.
    Picking(&'static str),
    /// A run that exited 0 and left a branch for a detached `HEAD`: `symbolic-ref -q HEAD`
    /// exited 0 before it and exits 1 after it.
    Detached,
    /// A run that exited 1.
    ExitedOne,
}

/// A question of the private repository's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Question {
    /// Whether `rebase-merge` or `rebase-apply` is a directory in its Git directory.
    Stopped,
    /// Whether `rev-parse -q --verify <name>` names a commit.
    Verifies(&'static str),
    /// Whether `symbolic-ref -q HEAD` finds `HEAD` on a branch, or detached.
    OnABranch,
}

/// A question's answer: exit 0, exit 1, or anything else, which answers nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Yes,
    No,
    Unanswered,
}

/// What may follow a run, decided from its words, and for a detached `HEAD` from the
/// state, before it starts.
pub struct Then(Option<&'static Family>);

/// What may follow the run of `words`, the command word first, against the repository
/// `against` names.
pub fn before(words: &[OsString], against: Against) -> Then {
    if against.private_repository().is_none() {
        return Then(None);
    }
    Then(before_asking(words, |question| asked(question, against)))
}

/// Writes the hint, after a run that ended as `ended`, when its family's end and state
/// hold.
pub fn after(then: Then, ended: &Outcome, against: Against) {
    let (Then(Some(family)), Outcome::Git(end)) = (then, ended) else {
        return;
    };
    if follows(family.trigger, end, |question| asked(question, against)) {
        lines::write(Level::Hint, family.line);
    }
}

/// The family of `words` when they admit its hint and, for a detached `HEAD`, `ask` finds
/// `HEAD` on a branch before the run.
fn before_asking(
    words: &[OsString],
    ask: impl FnOnce(Question) -> Answer,
) -> Option<&'static Family> {
    let family = admitted(words)?;
    match family.trigger {
        Trigger::Detached => (ask(Question::OnABranch) == Answer::Yes).then_some(family),
        _ => Some(family),
    }
}

/// The family whose word begins `words` when the rest admit its hint: no help request
/// and no excluding word before the first `--`, and a required word among them.
fn admitted(words: &[OsString]) -> Option<&'static Family> {
    let (command, rest) = words.split_first()?;
    let family = FAMILIES
        .iter()
        .find(|family| family.word == command.as_bytes())?;
    let mut before_the_end = rest
        .iter()
        .map(|word| word.as_bytes())
        .take_while(|word| *word != b"--");
    let kept_away = before_the_end
        .any(|word| matches!(word, b"-h" | b"--help") || family.excluding.contains(&word));
    let required = family.requiring.is_empty()
        || rest
            .iter()
            .any(|word| family.requiring.contains(&word.as_bytes()));
    (!kept_away && required).then_some(family)
}

/// Whether the hint follows a run that ended as `end`: `ask` is put a question only once
/// the end holds.
fn follows(trigger: Trigger, end: &End, ask: impl FnOnce(Question) -> Answer) -> bool {
    let exited_0 = *end == End::Code(0);
    match trigger {
        Trigger::Stopped => !exited_0 && ask(Question::Stopped) == Answer::Yes,
        Trigger::Picking(name) => !exited_0 && ask(Question::Verifies(name)) == Answer::Yes,
        Trigger::Detached => exited_0 && ask(Question::OnABranch) == Answer::No,
        Trigger::ExitedOne => *end == End::Code(1),
    }
}

/// The answer of the private repository `against` names.
fn asked(question: Question, against: Against) -> Answer {
    let words = match question {
        Question::Stopped => return stopped(against),
        Question::Verifies(name) => ["rev-parse", "-q", "--verify", name].to_vec(),
        Question::OnABranch => ["symbolic-ref", "-q", "HEAD"].to_vec(),
    };
    match Run::against(against, words).capture_output().start() {
        Ok(finished) if finished.end == End::Code(0) => Answer::Yes,
        Ok(finished) if finished.end == End::Code(1) => Answer::No,
        _ => Answer::Unanswered,
    }
}

/// Whether `lstat` finds `rebase-merge` or `rebase-apply` a directory in the private Git
/// directory; a symbolic link to one is none.
fn stopped(against: Against) -> Answer {
    let Some(git_directory) = against.private_repository() else {
        return Answer::Unanswered;
    };
    let directory = |name: &str| {
        fs::symlink_metadata(git_directory.join(name)).is_ok_and(|found| found.is_dir())
    };
    if directory("rebase-merge") || directory("rebase-apply") {
        Answer::Yes
    } else {
        Answer::No
    }
}

#[cfg(test)]
mod tests;
