//! The hint after a word passed through whose outcome Git's advice names a plain `git`
//! command for (`Holds/G19`, G25): `rebase` and `am` stopped, `rebase` and `merge` whose
//! autostash came back with conflicts, `cherry-pick` and `revert` stopped, `commit
//! --amend` refusing and `commit` refusing while a cherry-pick stands, `checkout` and
//! `switch` leaving a branch for a detached `HEAD`, and `branch -d` refusing. Copied under
//! `git dupe`, Git's command would act on the project's repository; the hint names the
//! `git dupe` form.
//!
//! What the code below cannot show:
//!
//! - Git's own output reaches the user untouched and is never read (R5, `Composition/Git`):
//!   each hint is decided from the run's end and the private repository's state, and
//!   written after every byte of Git's, before settle's warnings. It follows the state,
//!   not Git's `advice.*` settings, so it still follows where a setting silences Git's
//!   advice. The exit status is the run's (F9).
//! - The words read are the run's as typed, or those an alias chain reached (`alias`), up
//!   to their first `--`, after which a word is an operand. The words that keep the
//!   `checkout` and `switch` hint away are matched exactly: a bundle or an abbreviation of
//!   `--detach` or `-q` is not read, so a hint may follow where Git printed no advice,
//!   never one the state makes false. So are `commit`'s `--amend` and `--allow-empty`, of
//!   which `--allow-empty-message` is no spelling, and the `--quit` of `merge` and
//!   `rebase`. `cherry-pick`'s and `revert`'s `-n` is read in a bundle too
//!   (`reads_as_no_commit`), because the line's `--continue` would commit what it was
//!   given to leave uncommitted. `branch`'s `--delete` is read in every spelling Git
//!   reads as it (`reads_as_delete`), so a bundle that holds `d` as another option's value
//!   draws the hint where Git printed no advice; the line is true either way. None follows
//!   a run with `-h` or `--help` before the first `--` of the words read, which an
//!   expansion's own `--` can move a typed `-h` after, nor a run that did not start, nor
//!   one against a Git directory where no private repository stands, as where a help
//!   request runs in a worktree no `init` attached.
//! - A stopped `rebase` or `am` is decided by its directory, whatever the run's end: an
//!   `edit` or `break` stop of `rebase -i` exits 0, and an interrupted rebase is a stopped
//!   one. An `am` reads only `rebase-apply`, the one its own stop keeps: beside a rebase
//!   stopped in `rebase-merge`, its line would name commands that fail. "Did not exit 0"
//!   is any end but exit 0, a signal included. An end named by its number, 0 or 1, is
//!   never a signal's.
//! - Where a word has several entries, the first whose words, end, and state hold decides,
//!   in the table's order: a `rebase` stopped is decided by its directory before the
//!   autostash is asked about, and a `commit` by its `--amend` before a stopped
//!   cherry-pick is asked about. An entry's question is put only once the entries before
//!   it have not held, and a question that answers with any status but its two ends the
//!   decision with no hint, the command's status as it was.
//! - The runs an entry adds are fixed for its words (G22, R4): `checkout` and `switch` ask
//!   the private `symbolic-ref -q HEAD` before the run and, after one that exited 0, again,
//!   both only where the words admit the hint; `cherry-pick` and `revert` ask `rev-parse
//!   -q --verify` of their pseudo-reference after a run that did not exit 0, and `commit`
//!   without `--amend` asks it of `CHERRY_PICK_HEAD` alone after a run that exited 1;
//!   `merge`, and `rebase` where it left no stop, ask one `ls-files -u -z` after a run
//!   that exited 0, from the root, because `ls-files` lists only what lies below its
//!   directory; `am`, `branch`, and `commit --amend` add none. The `symbolic-ref` before
//!   the run decides nothing about the run (N8).
//! - An index entry left unmerged by a `rebase` or `merge` that exited 0 has one cause, an
//!   autostash that came back with conflicts and stays a stash entry (S12), whether
//!   `--autostash` or the configuration asked for it, so the index is asked and not the
//!   words; but for `--quit`, which ends the operation leaving its conflicts in the index
//!   and an autostash, where there is one, unapplied in the stash, so that the line and its
//!   `stash drop` would be false there. `merge` keeps no stop of its own, so its entry reads
//!   no directory.
//! - `CHERRY_PICK_HEAD` and `REVERT_HEAD` are asked of Git, because they live in the
//!   reference store, which may hold no file for them (S15); `rebase-merge` and
//!   `rebase-apply` are directories in every store, read by `lstat` (S12).
//! - A `commit` during a stopped revert gets no hint: Git's advice there names only the
//!   revert's own commands, which fail in the project's repository, and none of its
//!   own (S12).
//! - The `branch -d` hint follows exit 1 alone, so a branch that does not exist draws it
//!   too; it says what deletes a branch `-d` keeps, which is true either way. So does the
//!   `commit --amend` hint, a hook's or an editor's refusal included, and its line says
//!   what its commands do where the amend would have left the commit empty; it names
//!   `reset HEAD^` for a commit with a parent, because a root commit has none.

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use super::lines::{self, Level};
use super::outcome::Outcome;
use crate::runner::{Against, End, Run};

// `--skip` is described by the commit it leaves out: at an `edit`, `break`, or reword
// stop the commit stopped at is already made, and `--skip` keeps it.
const REBASE: &[u8] = b"a private rebase that stopped goes on with 'git dupe rebase --continue' \
    once 'git dupe add <path>' has marked each conflict resolved, \
    'git dupe commit --amend' has amended the commit an edit or a reword stopped at, or \
    'git dupe rebase --edit-todo' has mended a todo list Git could not read, as the stop \
    asks; 'git dupe rebase --skip' leaves out a commit that would not apply, and \
    'git dupe rebase --abort' ends the rebase; plain 'git add', 'git commit', and \
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
    once 'git dupe add <path>' has marked each conflict resolved, or, where the revert is \
    empty, with 'git dupe commit --allow-empty'; it skips the commit with \
    'git dupe revert --skip', or ends with 'git dupe revert --abort'; plain 'git add', \
    'git commit', and 'git revert' act on the project's repository";

// Conditioned: a branch left with no commit yet is none `switch -` can go back to.
const DETACHED: &[u8] = b"HEAD is now detached in the private repository: \
    'git dupe switch -c <new-branch-name>' keeps commits made here on a new branch, and \
    'git dupe switch -' goes back to the branch left, where that branch has a commit; plain \
    'git switch' acts on the project's repository";

const UNMERGED: &[u8] = b"'git dupe branch -D <branch>' deletes a private branch that \
    'git dupe branch -d' keeps because it is not fully merged; plain 'git branch -D' acts on \
    the project's repository";

/// The line after a run whose autostash came back with conflicts: its one text.
pub(super) const AUTOSTASH: &[u8] = b"the private changes Git stashed before merging or \
    rebasing came back with conflicts, and the stash entry still holds them: once \
    'git dupe add <path>' has marked each conflict resolved, 'git dupe stash drop' drops \
    the entry; or 'git dupe reset --hard' throws away the changes that came back, \
    conflicts and all, and 'git dupe stash pop' applies the entry again later; plain \
    'git add', 'git stash', and 'git reset' act on the project's repository";

// Conditioned: exit 1 alone does not prove the amend empty. A root commit has no `HEAD^`.
const EMPTY_AMEND: &[u8] = b"where the private amend would have left the commit empty, \
    'git dupe commit --amend --allow-empty' amends it all the same, and, on a commit with a \
    parent, 'git dupe reset HEAD^' removes the commit, leaving the files as they are; \
    plain 'git commit' and 'git reset' act on the project's repository";

/// The entries of the table, in its order: where a word has several, the first whose
/// words, end, and state hold decides.
const FAMILIES: [Family; 11] = [
    Family {
        word: b"rebase",
        excluding: None,
        requiring: None,
        trigger: Trigger::Stopped(&["rebase-merge", "rebase-apply"]),
        line: REBASE,
    },
    Family {
        word: b"rebase",
        excluding: Some(quits),
        requiring: None,
        trigger: Trigger::Unmerged,
        line: AUTOSTASH,
    },
    Family {
        word: b"merge",
        excluding: Some(quits),
        requiring: None,
        trigger: Trigger::Unmerged,
        line: AUTOSTASH,
    },
    Family {
        word: b"am",
        excluding: None,
        requiring: None,
        trigger: Trigger::Stopped(&["rebase-apply"]),
        line: AM,
    },
    Family {
        word: b"cherry-pick",
        excluding: Some(reads_as_no_commit),
        requiring: None,
        trigger: Trigger::Picking("CHERRY_PICK_HEAD"),
        line: CHERRY_PICK,
    },
    Family {
        word: b"revert",
        excluding: Some(reads_as_no_commit),
        requiring: None,
        trigger: Trigger::Picking("REVERT_HEAD"),
        line: REVERT,
    },
    Family {
        word: b"commit",
        excluding: Some(|word| word == b"--allow-empty"),
        requiring: Some(|word| word == b"--amend"),
        trigger: Trigger::ExitedOne,
        line: EMPTY_AMEND,
    },
    Family {
        word: b"commit",
        excluding: Some(|word| word == b"--amend"),
        requiring: None,
        trigger: Trigger::ExitedOneWhile("CHERRY_PICK_HEAD"),
        line: CHERRY_PICK,
    },
    Family {
        word: b"checkout",
        excluding: Some(|word| matches!(word, b"--detach" | b"-q" | b"--quiet")),
        requiring: None,
        trigger: Trigger::Detached,
        line: DETACHED,
    },
    Family {
        word: b"switch",
        excluding: Some(|word| matches!(word, b"--detach" | b"-d" | b"-q" | b"--quiet")),
        requiring: None,
        trigger: Trigger::Detached,
        line: DETACHED,
    },
    Family {
        word: b"branch",
        excluding: None,
        requiring: Some(reads_as_delete),
        trigger: Trigger::ExitedOne,
        line: UNMERGED,
    },
];

/// A command word whose run Git's advice may follow, and when the hint follows it: one
/// entry of the table.
struct Family {
    word: &'static [u8],
    /// What no word before the first `--` may spell, when anything: Git prints no such
    /// advice after it.
    excluding: Option<fn(&[u8]) -> bool>,
    /// What one word before the first `--` must spell, when anything.
    requiring: Option<fn(&[u8]) -> bool>,
    trigger: Trigger,
    line: &'static [u8],
}

/// The run's end and the state after which a family's hint follows (S12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    /// A run of any end, while one of these is a directory in the private Git directory.
    Stopped(&'static [&'static str]),
    /// A run that did not exit 0, while `rev-parse -q --verify` of this name exits 0.
    Picking(&'static str),
    /// A run that exited 1, while `rev-parse -q --verify` of this name exits 0.
    ExitedOneWhile(&'static str),
    /// A run that exited 0, while `ls-files -u -z` lists an unmerged entry.
    Unmerged,
    /// A run that exited 0 and left a branch for a detached `HEAD`: `symbolic-ref -q HEAD`
    /// exited 0 before it and exits 1 after it.
    Detached,
    /// A run that exited 1.
    ExitedOne,
}

/// A question of the private repository's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Question {
    /// Whether one of these is a directory in its Git directory.
    Stopped(&'static [&'static str]),
    /// Whether `rev-parse -q --verify <name>` names a commit.
    Verifies(&'static str),
    /// Whether `symbolic-ref -q HEAD` finds `HEAD` on a branch, or detached.
    OnABranch,
    /// Whether `ls-files -u -z` lists an unmerged entry.
    Unmerged,
}

/// A question's answer: yes, no, or a run that answered neither, which answers nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Answer {
    Yes,
    No,
    Unanswered,
}

/// What may follow a run, decided from its words, and for a detached `HEAD` from the
/// state, before it starts: the entries the words admit, in the table's order.
pub struct Then(Vec<&'static Family>);

/// What may follow the run of `words`, the command word first, against the repository
/// `against` names.
pub fn before(words: &[OsString], against: Against) -> Then {
    if against.private_repository().is_none() {
        return Then(Vec::new());
    }
    Then(before_asking(words, |question| asked(question, against)))
}

/// Writes the hint, after a run that ended as `ended`, of the first entry whose end and
/// state hold.
pub fn after(then: Then, ended: &Outcome, against: Against) {
    let Outcome::Git(end) = ended else {
        return;
    };
    if let Some(line) = decided(&then.0, end, |question| asked(question, against)) {
        lines::write(Level::Hint, line);
    }
}

/// The entries `words` admit, those for a detached `HEAD` only where `ask` finds `HEAD` on
/// a branch before the run.
fn before_asking(words: &[OsString], ask: impl FnOnce(Question) -> Answer) -> Vec<&'static Family> {
    let mut families = admitted(words);
    let detached = |family: &&Family| family.trigger == Trigger::Detached;
    if families.iter().any(detached) && ask(Question::OnABranch) != Answer::Yes {
        families.retain(|family| !detached(family));
    }
    families
}

/// The entries whose word begins `words` and whose hint the rest admit, in the table's
/// order: before the first `--`, no help request, no word an entry excludes, and a word
/// that spells what it requires.
fn admitted(words: &[OsString]) -> Vec<&'static Family> {
    let Some((command, rest)) = words.split_first() else {
        return Vec::new();
    };
    let before_the_end: Vec<&[u8]> = rest
        .iter()
        .map(|word| word.as_bytes())
        .take_while(|word| *word != b"--")
        .collect();
    let spelled = |spells: fn(&[u8]) -> bool| before_the_end.iter().any(|word| spells(word));
    if spelled(|word| matches!(word, b"-h" | b"--help")) {
        return Vec::new();
    }
    FAMILIES
        .iter()
        .filter(|family| family.word == command.as_bytes())
        .filter(|family| !family.excluding.is_some_and(spelled))
        .filter(|family| family.requiring.is_none_or(spelled))
        .collect()
}

/// Whether `merge` or `rebase` reads `word` as `--quit`, which exits 0 with the conflicts
/// it leaves unmerged and no autostash applied; an abbreviation is ambiguous with
/// `--quiet`, which Git refuses, exit 129.
fn quits(word: &[u8]) -> bool {
    word == b"--quit"
}

/// Whether `cherry-pick` or `revert` reads `word` as `--no-commit`: the word itself, or
/// `-n` alone or bundled before any of `m`, `X`, and `S`, which take the rest of the word
/// as their value. A prefix of `--no-commit` is read as another option or refused.
fn reads_as_no_commit(word: &[u8]) -> bool {
    if word == b"--no-commit" {
        return true;
    }
    match word.strip_prefix(b"-") {
        Some(letters) if !letters.starts_with(b"-") => letters
            .iter()
            .take_while(|letter| !matches!(letter, b'm' | b'X' | b'S'))
            .any(|letter| *letter == b'n'),
        _ => false,
    }
}

/// Whether `branch` reads `word` as `--delete`, or refuses it as ambiguous, exit 129, in
/// every listed release (S12): `--delete` or a prefix of it at least three bytes long, or
/// a word of short options, beginning with one `-`, that holds `d` and not `D`, which
/// would force the deletion.
fn reads_as_delete(word: &[u8]) -> bool {
    match word.strip_prefix(b"--") {
        Some(long) => !long.is_empty() && b"delete".starts_with(long),
        None => word.starts_with(b"-") && word.contains(&b'd') && !word.contains(&b'D'),
    }
}

/// The line of the first of `families` whose end and state hold after a run that ended as
/// `end`: each entry's question is put to `ask` in turn, and one that answers nothing ends
/// the decision with none.
fn decided(
    families: &[&'static Family],
    end: &End,
    mut ask: impl FnMut(Question) -> Answer,
) -> Option<&'static [u8]> {
    for family in families {
        let mut unanswered = false;
        let holds = follows(family.trigger, end, |question| {
            let answer = ask(question);
            unanswered = answer == Answer::Unanswered;
            answer
        });
        if holds {
            return Some(family.line);
        }
        if unanswered {
            return None;
        }
    }
    None
}

/// Whether the hint follows a run that ended as `end`: `ask` is put a question only once
/// the end holds.
fn follows(trigger: Trigger, end: &End, ask: impl FnOnce(Question) -> Answer) -> bool {
    let exited_0 = *end == End::Code(0);
    let exited_1 = *end == End::Code(1);
    match trigger {
        Trigger::Stopped(directories) => ask(Question::Stopped(directories)) == Answer::Yes,
        Trigger::Picking(name) => !exited_0 && ask(Question::Verifies(name)) == Answer::Yes,
        Trigger::ExitedOneWhile(name) => exited_1 && ask(Question::Verifies(name)) == Answer::Yes,
        Trigger::Unmerged => exited_0 && ask(Question::Unmerged) == Answer::Yes,
        Trigger::Detached => exited_0 && ask(Question::OnABranch) == Answer::No,
        Trigger::ExitedOne => exited_1,
    }
}

/// The answer of the private repository `against` names.
fn asked(question: Question, against: Against) -> Answer {
    let words = match question {
        Question::Stopped(directories) => return stopped(against, directories),
        Question::Unmerged => return unmerged(against),
        Question::Verifies(name) => ["rev-parse", "-q", "--verify", name].to_vec(),
        Question::OnABranch => ["symbolic-ref", "-q", "HEAD"].to_vec(),
    };
    match Run::against(against, words).capture_output().start() {
        Ok(finished) if finished.end == End::Code(0) => Answer::Yes,
        Ok(finished) if finished.end == End::Code(1) => Answer::No,
        _ => Answer::Unanswered,
    }
}

/// Whether `rev-parse -q --verify <name>` names a commit in the private repository
/// `against` names: `HEAD` on a branch with one, or a pick's pseudo-reference (S12).
pub(super) fn verifies(name: &'static str, against: Against) -> Answer {
    if against.private_repository().is_none() {
        return Answer::Unanswered;
    }
    asked(Question::Verifies(name), against)
}

/// Whether the index of the private repository `against` names holds an unmerged entry:
/// one `ls-files -u -z` from the root, answering by whether it lists anything once it has
/// exited 0. The one reader of unmerged entries.
pub(super) fn unmerged(against: Against) -> Answer {
    let Against::Private { root, .. } = against else {
        return Answer::Unanswered;
    };
    match Run::against(against, ["ls-files", "-u", "-z"])
        .from(root)
        .capture_output()
        .start()
    {
        Ok(finished) if finished.end == End::Code(0) => {
            if finished.stdout.is_empty() {
                Answer::No
            } else {
                Answer::Yes
            }
        }
        _ => Answer::Unanswered,
    }
}

/// Whether `lstat` finds one of `directories` a directory in the private Git directory; a
/// symbolic link to one is none.
fn stopped(against: Against, directories: &[&str]) -> Answer {
    let Some(git_directory) = against.private_repository() else {
        return Answer::Unanswered;
    };
    let directory = |name: &&str| {
        fs::symlink_metadata(git_directory.join(name)).is_ok_and(|found| found.is_dir())
    };
    if directories.iter().any(directory) {
        Answer::Yes
    } else {
        Answer::No
    }
}

#[cfg(test)]
mod tests;
