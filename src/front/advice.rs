//! The hint after a word passed through whose outcome Git's advice names a plain `git`
//! command for (`Holds/G19`, G25): `rebase` and `am` stopped, `rebase` and `merge` whose
//! autostash came back with conflicts, a `merge` that left a merge in progress,
//! `cherry-pick` and `revert` stopped, `commit --amend` refusing and `commit` refusing
//! while a cherry-pick or a rebase's pick stands, `checkout` and `switch` leaving a branch
//! for a detached `HEAD` or, given `-m`, going to another branch with conflicts, and
//! `branch -d` refusing. Copied under `git dupe`, Git's command would act on the project's
//! repository; the hint names the `git dupe` form. The hint after a `mv` of a directory
//! `.gitdupe` lists, which answers no advice of Git's, is `rename`'s.
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
//!   never one the state makes false. So are `commit`'s `--allow-empty`, of which
//!   `--allow-empty-message` is no spelling, and `--dry-run`, under which the named command
//!   would amend nothing, and the `--quit` of `merge` and `rebase`.
//!   `commit`'s `--amend` is read in every prefix Git reads as it (`reads_as_amend`), and
//!   the `-m` of `checkout` and `switch` in a bundle too (`reads_as_merge`). `cherry-pick`'s and `revert`'s `-n` is read in a bundle too
//!   (`reads_as_no_commit`), because the line's `--continue` would commit what it was
//!   given to leave uncommitted. `branch`'s `--delete` is read in every spelling Git
//!   reads as it (`reads_as_delete`), so a bundle that holds `d` as another option's value
//!   draws the hint where Git printed no advice; the line is true either way. None follows
//!   a run with `-h` or `--help` before the first `--` of the words read, which an
//!   expansion's own `--` can move a typed `-h` after, nor a run that did not start, nor
//!   one against a Git directory where no private repository stands, as where a help
//!   request runs in a worktree no `init` attached.
//! - A stopped `rebase` or `am` is decided by what its own stop keeps, whatever the run's
//!   end: an `edit` or `break` stop of `rebase -i` exits 0, and an interrupted rebase is a
//!   stopped one. A rebase keeps `rebase-merge`, or `rebase-apply` under `--apply`, and an
//!   `am` keeps `rebase-apply` too, each marking `rebase-apply` with a file of its own,
//!   `rebasing` or `applying` (S12): beside the other's stop, a line would name commands
//!   that fail. "Did not exit 0" is any end but exit 0, a signal included. An end named by
//!   its number, 0 or 1, is never a signal's.
//! - Where a word has several entries, the first whose words, end, and state hold decides,
//!   in the table's order: a `rebase` stopped is decided by its directory before the
//!   autostash is asked about, and a `commit` by its `--amend` before a stopped
//!   cherry-pick is asked about. An entry's question is put only once the entries before
//!   it have not held, and a question that answers with any status but its two ends the
//!   decision with no hint, the command's status as it was.
//! - The runs an entry adds are fixed for its words (G22, R4): `checkout` and `switch` ask
//!   the private `symbolic-ref -q HEAD` before the run and, after one that exited 0, once
//!   again, both only where the words admit a hint, the one answer after the run serving
//!   the detached `HEAD` and `-m` alike; `cherry-pick` and `revert` ask `rev-parse -q
//!   --verify` of their pseudo-reference, and `merge` of `MERGE_HEAD`, after a run that did
//!   not exit 0, and `commit` without `--amend` asks it of `CHERRY_PICK_HEAD` alone after a
//!   run that exited 1; `merge`, `rebase` where it left no stop, and `checkout` and
//!   `switch` given `-m` where no detached `HEAD` decided and `HEAD` moved, ask one
//!   `ls-files -u -z` after a run that exited 0, from the root, because `ls-files` lists
//!   only what lies below its directory; `am`, `branch`, and `commit --amend` add none. The
//!   `symbolic-ref` before the run decides nothing about the run (N8).
//! - An index entry left unmerged by a `rebase` or `merge` that exited 0 has one cause, an
//!   autostash that came back with conflicts and stays a stash entry (S12), whether
//!   `--autostash` or the configuration asked for it, so the index is asked and not the
//!   words; but for `--quit`, which ends the operation leaving its conflicts in the index
//!   and an autostash, where there is one, unapplied in the stash, so that the line and its
//!   `stash drop` would be false there. `merge` keeps no stop directory of its own, so its
//!   autostash entry reads none. After `checkout -m` or `switch -m` such an entry is a local
//!   change whose merge conflicted, which only from 2.55.0 Git keeps as a stash entry (S12):
//!   its line is conditioned on that entry, which nothing it reads can prove. It follows
//!   only where `HEAD` names another branch after the run than before it, or moved onto or
//!   off a branch: a `checkout -m` given paths, which re-creates their conflicts and
//!   switches nothing, carried no change and kept no entry, however its words are spelled.
//! - A `merge` that did not exit 0 has left a merge in progress exactly where `MERGE_HEAD`
//!   names a commit after it (S12): a conflict, and a second `merge` Git refused because
//!   the first stands; one Git refused before starting, over local changes or a word that
//!   names no commit, and a `--squash`, write none. A rebase stopped beside it is the
//!   rebase's to go on with.
//! - `CHERRY_PICK_HEAD` and `REVERT_HEAD` are asked of Git, because they live in the
//!   reference store, which may hold no file for them (S15); a rebase's stop is in the
//!   Git directory in every store, read by `lstat` (S12), after the pseudo-reference, so
//!   that the runs stay fixed for the words and the end.
//! - A rebase stopped at a pick that became empty keeps `CHERRY_PICK_HEAD` beside its stop
//!   (S12): a `cherry-pick` or `revert` run there has stopped nothing of its own and draws
//!   no hint, and a `commit` there draws the rebase's line, Git's advice naming
//!   `git rebase --skip`. A stopped `am`, which keeps `rebase-apply` holding `applying`,
//!   is no rebase: a pick stopped beside it, and a `commit` while it stands, draw the
//!   pick's line, whose commands Git runs while the `am` stands.
//! - A `commit` during a stopped revert gets no hint: Git's advice there names only the
//!   revert's own commands, which fail in the project's repository, and none of its
//!   own (S12).
//! - The `branch -d` hint follows exit 1 alone, so a branch that does not exist draws it
//!   too; it says what deletes a branch `-d` keeps, which is true either way. So does the
//!   `commit --amend` hint, a hook's or an editor's refusal included, and its line says
//!   what its commands do where the amend would have left the commit empty; it names
//!   `--allow-empty` added to the command as typed, because a bare form would drop an
//!   `--only <path>` and commit what is staged, and `reset HEAD^` for a commit with a
//!   parent, because a root commit has none.

use std::fs;
use std::path::Path;

use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use crate::runner::{Against, End, Run};

// `--skip` is described by the commit it leaves out: at an `edit`, `break`, or reword
// stop the commit stopped at is already made, and `--skip` keeps it.
pub(super) const REBASE: &[u8] =
    b"a private rebase that stopped goes on with 'git dupe rebase --continue' \
    once 'git dupe add <path>' has marked each conflict resolved, \
    'git dupe commit --amend' has amended the commit an edit or a reword stopped at, \
    'git dupe commit --allow-empty' has kept a commit the replay left empty, or \
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

/// The line after a run that left a private merge in progress: its one text, which a
/// `pull` that stopped the same way shares.
pub(super) const MERGE: &[u8] = b"a private merge is in progress: once 'git dupe add <path>' \
    has marked each conflict resolved, 'git dupe merge --continue' concludes it, and \
    'git dupe merge --abort' gives it up; plain 'git add', 'git commit', and 'git merge' act \
    on the project's repository";

/// The line after a run whose autostash came back with conflicts: its one text.
pub(super) const AUTOSTASH: &[u8] = b"the private changes Git stashed before merging or \
    rebasing came back with conflicts, and the stash entry still holds them: once \
    'git dupe add <path>' has marked each conflict resolved, 'git dupe stash drop' drops \
    the entry; or 'git dupe reset --hard' throws away the changes that came back, \
    conflicts and all, and 'git dupe stash pop' applies the entry again later; plain \
    'git add', 'git stash', and 'git reset' act on the project's repository";

// Conditioned: exit 1 alone does not prove the amend empty. A root commit has no `HEAD^`.
const EMPTY_AMEND: &[u8] = b"where the private amend would have left the commit empty, the \
    same 'git dupe commit' as typed with --allow-empty added amends it all the same, and, on \
    a commit with a parent, 'git dupe reset HEAD^' removes the commit, leaving the files as \
    they are; plain 'git commit' and 'git reset' act on the project's repository";

// Conditioned: earlier releases keep no stash entry here, so that `reset --hard` would
// lose the change for good and `stash drop` has nothing to drop.
const MERGED_CHANGES: &[u8] = b"the private changes carried across the switch came back with \
    conflicts: 'git dupe add <path>' marks each one resolved; where Git also kept them as a \
    stash entry, as it does from 2.55.0, 'git dupe stash drop' then drops the entry, or \
    'git dupe reset --hard' throws away the changes that came back, conflicts and all, and \
    'git dupe stash pop' applies the entry again later; plain 'git add', 'git stash', and \
    'git reset' act on the project's repository";

/// The entries of the table, in its order: where a word has several, the first whose
/// words, end, and state hold decides.
const FAMILIES: [Family; 14] = [
    Family {
        word: b"rebase",
        excluding: None,
        requiring: None,
        trigger: Trigger::Stopped(A_REBASE_STOPPED),
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
        word: b"merge",
        excluding: None,
        requiring: None,
        trigger: Trigger::InProgress("MERGE_HEAD"),
        line: MERGE,
    },
    Family {
        word: b"am",
        excluding: None,
        requiring: None,
        trigger: Trigger::Stopped(AN_AM_STOPPED),
        line: AM,
    },
    Family {
        word: b"cherry-pick",
        excluding: Some(reads_as_no_commit),
        requiring: None,
        trigger: Trigger::InProgress("CHERRY_PICK_HEAD"),
        line: CHERRY_PICK,
    },
    Family {
        word: b"revert",
        excluding: Some(reads_as_no_commit),
        requiring: None,
        trigger: Trigger::InProgress("REVERT_HEAD"),
        line: REVERT,
    },
    Family {
        word: b"commit",
        excluding: Some(|word| matches!(word, b"--allow-empty" | b"--dry-run")),
        requiring: Some(reads_as_amend),
        trigger: Trigger::ExitedOne,
        line: EMPTY_AMEND,
    },
    Family {
        word: b"commit",
        excluding: Some(reads_as_amend),
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
    Family {
        word: b"checkout",
        excluding: None,
        requiring: Some(reads_as_merge),
        trigger: Trigger::Carried,
        line: MERGED_CHANGES,
    },
    Family {
        word: b"switch",
        excluding: None,
        requiring: Some(reads_as_merge),
        trigger: Trigger::Carried,
        line: MERGED_CHANGES,
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

/// What `lstat` finds in the private Git directory where a stop stands (S12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marker {
    /// A directory of this name.
    Directory(&'static str),
    /// A directory of the first name holding a file of the second.
    Holding(&'static str, &'static str),
}

/// A rebase stopped: `rebase-merge`, or `rebase-apply` holding a file `rebasing`.
const A_REBASE_STOPPED: &[Marker] = &[
    Marker::Directory("rebase-merge"),
    Marker::Holding("rebase-apply", "rebasing"),
];

/// An `am` stopped: `rebase-apply` holding a file `applying`.
const AN_AM_STOPPED: &[Marker] = &[Marker::Holding("rebase-apply", "applying")];

/// The run's end and the state after which a family's hint follows (S12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    /// A run of any end, while one of these markers stands.
    Stopped(&'static [Marker]),
    /// A run that did not exit 0, while `rev-parse -q --verify` of this name exits 0 — a
    /// pick, a revert, or a merge in progress — and no rebase stands stopped.
    InProgress(&'static str),
    /// A run that exited 1, while `rev-parse -q --verify` of this name exits 0: the
    /// rebase's line where a rebase stands stopped, and the family's otherwise.
    ExitedOneWhile(&'static str),
    /// A run that exited 0, while `ls-files -u -z` lists an unmerged entry.
    Unmerged,
    /// A run that exited 0 and left a branch for a detached `HEAD`: `symbolic-ref -q HEAD`
    /// exited 0 before it and exits 1 after it.
    Detached,
    /// A run that exited 0 and moved `HEAD`, `symbolic-ref -q HEAD` answering otherwise
    /// after it than before it, while `ls-files -u -z` lists an unmerged entry.
    Carried,
    /// A run that exited 1.
    ExitedOne,
}

/// A question of the private repository's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Question {
    /// Whether one of these markers stands in its Git directory.
    Stopped(&'static [Marker]),
    /// Whether `rev-parse -q --verify <name>` names a commit.
    Verifies(&'static str),
    /// Whether `symbolic-ref -q HEAD` finds `HEAD` on a branch, or detached.
    OnABranch,
    /// Whether `symbolic-ref -q HEAD` answers otherwise after the run than before it.
    Moved,
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

/// Where `HEAD` stands, as `symbolic-ref -q HEAD` answers (S12): the branch it names, an
/// unborn one included, or detached; and, where an entry asks, the commit it names, as
/// `rev-parse -q --verify HEAD` answers, none on an unborn branch.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Standing {
    head: Head,
    commit: Option<Option<Vec<u8>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Head {
    On(Vec<u8>),
    Detached,
}

/// What may follow a run, decided from its words, and for the entries that read `HEAD`
/// from the state, before it starts: the entries the words admit, in the table's order,
/// and where `HEAD` stood when one of them reads it.
pub struct Then {
    families: Vec<&'static Family>,
    standing: Option<Standing>,
}

impl Then {
    /// Whether an entry reads the commit `HEAD` names: one for changes carried across
    /// a move of `HEAD`, which a branch's line alone does not show (`checkout -m` to
    /// another commit from a detached `HEAD`, or with `-B` of the branch it is on).
    fn reads_the_commit(&self) -> bool {
        self.families
            .iter()
            .any(|family| family.trigger == Trigger::Carried)
    }
}

/// What may follow the run of `words`, the command word first, against the repository
/// `against` names.
pub fn before(words: &[Vec<u8>], against: Against) -> Then {
    if against.private_repository().is_none() {
        return Then {
            families: Vec::new(),
            standing: None,
        };
    }
    before_asking(words, |commit| standing(against, commit))
}

/// Writes the hint, after a run that ended as `ended`, of the first entry whose end and
/// state hold. Where `HEAD` stands after the run is asked once, whichever entries read it.
pub fn after(then: Then, ended: &Outcome, against: Against) {
    let Outcome::Git(end) = ended else {
        return;
    };
    let commit = then.reads_the_commit();
    let mut after_the_run: Option<Option<Standing>> = None;
    let line = decided(&then.families, end, |question| {
        let mut now = || {
            after_the_run
                .get_or_insert_with(|| standing(against, commit))
                .clone()
        };
        match question {
            Question::OnABranch => match now() {
                Some(Standing {
                    head: Head::On(_), ..
                }) => Answer::Yes,
                Some(_) => Answer::No,
                None => Answer::Unanswered,
            },
            Question::Moved => match (now(), &then.standing) {
                (Some(now), Some(before)) if now == *before => Answer::No,
                (Some(_), Some(_)) => Answer::Yes,
                _ => Answer::Unanswered,
            },
            other => asked(other, against),
        }
    });
    if let Some(line) = line {
        lines::write(Level::Hint, line);
    }
}

/// The entries `words` admit, and, where one of them reads `HEAD`, where `standing` finds
/// it before the run, the commit too where an entry reads it: those for a detached `HEAD`
/// only where it is on a branch, and none that reads it where `standing` answers nothing.
fn before_asking(words: &[Vec<u8>], standing: impl FnOnce(bool) -> Option<Standing>) -> Then {
    let mut then = Then {
        families: admitted(words),
        standing: None,
    };
    let reads_head =
        |family: &&Family| matches!(family.trigger, Trigger::Detached | Trigger::Carried);
    if !then.families.iter().any(reads_head) {
        return then;
    }
    let found = standing(then.reads_the_commit());
    then.families
        .retain(|family| match (family.trigger, &found) {
            (
                Trigger::Detached,
                Some(Standing {
                    head: Head::On(_), ..
                }),
            )
            | (Trigger::Carried, Some(_)) => true,
            (Trigger::Detached | Trigger::Carried, _) => false,
            _ => true,
        });
    then.standing = found;
    then
}

/// The entries whose word begins `words` and whose hint the rest admit, in the table's
/// order: before the first `--`, no help request, no word an entry excludes, and a word
/// that spells what it requires.
fn admitted(words: &[Vec<u8>]) -> Vec<&'static Family> {
    let Some((command, rest)) = words.split_first() else {
        return Vec::new();
    };
    let before_the_end: Vec<&[u8]> = rest
        .iter()
        .map(|word| word.as_slice())
        .take_while(|word| *word != b"--")
        .collect();
    let spelled = |spells: fn(&[u8]) -> bool| before_the_end.iter().any(|word| spells(word));
    if spelled(|word| matches!(word, b"-h" | b"--help")) {
        return Vec::new();
    }
    FAMILIES
        .iter()
        .filter(|family| family.word == command.as_slice())
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

/// Whether `commit` reads `word` as `--amend`, in every listed release (S12): the word, or
/// a prefix of it at least four bytes long. `--a` is ambiguous, exit 129.
fn reads_as_amend(word: &[u8]) -> bool {
    word.len() >= 4 && b"--amend".starts_with(word)
}

/// Whether `checkout` or `switch` reads `word` as `--merge`: the word itself, or a word
/// beginning with one `-` that holds `m` among its letters, which a value attached to
/// another letter can also spell, the line being true either way.
fn reads_as_merge(word: &[u8]) -> bool {
    word == b"--merge"
        || word
            .strip_prefix(b"-")
            .is_some_and(|letters| !letters.starts_with(b"-") && letters.contains(&b'm'))
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
/// `end`: each entry's questions are put to `ask` in turn, and one that answers nothing
/// ends the decision with none.
fn decided(
    families: &[&'static Family],
    end: &End,
    mut ask: impl FnMut(Question) -> Answer,
) -> Option<&'static [u8]> {
    for family in families {
        let mut unanswered = false;
        let line = follows(family, end, |question| {
            let answer = ask(question);
            unanswered = answer == Answer::Unanswered;
            answer
        });
        if line.is_some() || unanswered {
            return line;
        }
    }
    None
}

/// The line that follows a run that ended as `end` where `family`'s end and state hold:
/// `ask` is put a question only once the end holds, and each later one only once those
/// before it have answered as the entry needs.
fn follows(
    family: &Family,
    end: &End,
    mut ask: impl FnMut(Question) -> Answer,
) -> Option<&'static [u8]> {
    let exited_0 = *end == End::Code(0);
    let exited_1 = *end == End::Code(1);
    let holds = match family.trigger {
        Trigger::Stopped(markers) => ask(Question::Stopped(markers)) == Answer::Yes,
        Trigger::InProgress(name) => {
            !exited_0
                && ask(Question::Verifies(name)) == Answer::Yes
                && ask(Question::Stopped(A_REBASE_STOPPED)) == Answer::No
        }
        Trigger::ExitedOneWhile(name) => {
            if !exited_1 || ask(Question::Verifies(name)) != Answer::Yes {
                return None;
            }
            match ask(Question::Stopped(A_REBASE_STOPPED)) {
                Answer::Yes => return Some(REBASE),
                Answer::No => true,
                Answer::Unanswered => false,
            }
        }
        Trigger::Unmerged => exited_0 && ask(Question::Unmerged) == Answer::Yes,
        Trigger::Detached => exited_0 && ask(Question::OnABranch) == Answer::No,
        Trigger::Carried => {
            exited_0
                && ask(Question::Moved) == Answer::Yes
                && ask(Question::Unmerged) == Answer::Yes
        }
        Trigger::ExitedOne => exited_1,
    };
    holds.then_some(family.line)
}

/// The answer of the private repository `against` names to a question that is not where
/// `HEAD` stands, which `after` answers.
fn asked(question: Question, against: Against) -> Answer {
    let name = match question {
        Question::Stopped(markers) => return stopped(against, markers),
        Question::Unmerged => return unmerged(against),
        Question::Verifies(name) => name,
        Question::OnABranch | Question::Moved => return Answer::Unanswered,
    };
    match Run::against(against, ["rev-parse", "-q", "--verify", name])
        .capture_output()
        .start()
    {
        Ok(finished) if finished.end == End::Code(0) => Answer::Yes,
        Ok(finished) if finished.end == End::Code(1) => Answer::No,
        _ => Answer::Unanswered,
    }
}

/// Where `HEAD` stands in the private repository `against` names, the commit it names too
/// where `commit` asks: any answer of either run but its two answers nothing.
fn standing(against: Against, commit: bool) -> Option<Standing> {
    let head = match head_line(against) {
        Ok(Some(line)) => Head::On(line),
        Ok(None) => Head::Detached,
        Err(_) => return None,
    };
    let commit = match commit {
        false => None,
        true => {
            let finished = Run::against(against, ["rev-parse", "-q", "--verify", "HEAD"])
                .capture_output()
                .start()
                .ok()?;
            match finished.end {
                End::Code(0) => Some(Some(finished.stdout)),
                End::Code(1) => Some(None),
                _ => return None,
            }
        }
    };
    Some(Standing { head, commit })
}

/// The line of one private `symbolic-ref -q HEAD` in the repository `against` names: the
/// branch `HEAD` is on, an unborn one included, exit 0, or none where it is detached, exit
/// 1 (S12); an empty line on a branch cannot be read. The one reader of where `HEAD`
/// stands, `transfer`'s too.
pub(super) fn head_line(against: Against) -> Result<Option<Vec<u8>>, Outcome> {
    let run = Run::against(against, ["symbolic-ref", "-q", "HEAD"]).capture_output();
    let finished = run.start().map_err(outcome::not_started)?;
    match finished.end {
        End::Code(0) if finished.stdout.is_empty() => Err(outcome::unreadable()),
        End::Code(0) => Ok(Some(finished.stdout)),
        End::Code(1) => Ok(None),
        end => Err(Outcome::Git(end)),
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

/// Whether a rebase stands stopped in the private repository `against` names, read by
/// `lstat`, no Git run.
pub(super) fn rebase_stopped(against: Against) -> Answer {
    stopped(against, A_REBASE_STOPPED)
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

/// Whether one of `markers` stands in the Git directory of the private repository `against`
/// names.
fn stopped(against: Against, markers: &[Marker]) -> Answer {
    match against.private_repository() {
        Some(git_directory) if marked(git_directory, markers) => Answer::Yes,
        Some(_) => Answer::No,
        None => Answer::Unanswered,
    }
}

/// Whether `lstat` finds one of `markers` in `git_directory`: a directory where it names
/// one, and a regular file in it where it names that too; a symbolic link is neither.
fn marked(git_directory: &Path, markers: &[Marker]) -> bool {
    let found = |path: &Path| fs::symlink_metadata(path).ok();
    markers.iter().any(|marker| match *marker {
        Marker::Directory(name) => found(&git_directory.join(name)).is_some_and(|it| it.is_dir()),
        Marker::Holding(name, file) => {
            let directory = git_directory.join(name);
            found(&directory).is_some_and(|it| it.is_dir())
                && found(&directory.join(file)).is_some_and(|it| it.is_file())
        }
    })
}

#[cfg(test)]
mod tests;
