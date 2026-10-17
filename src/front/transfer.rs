//! The guard of the commands that move private history or configure where it goes
//! (G18, `Holds/G18`): `push`, `pull`, `fetch`, and `remote`, each run as Git itself once
//! nothing it would reach names a public place.
//!
//! What the code below cannot show:
//!
//! - Every word after the command is compared, and, for a word holding `=`, the part
//!   after its first `=`: no form of the commands (`remote add`, `set-url`, `--push`,
//!   `--mirror=`, a refspec, an option's value) is read, so that none can carry a
//!   destination past the guard. A help request among the words changes nothing in the
//!   comparison, because Git may read `-h` as an option's value.
//! - `push`, `pull`, and `fetch` are also compared by what Git reaches without a typed
//!   destination (`guards::transfer`): first every URL the private configuration gives
//!   any remote, whatever destination is typed, its refusal naming the remote and `git
//!   dupe remote remove`; then every member of every remote group, each as a typed word
//!   is compared, whatever the words name, because `fetch` and `pull` run against each
//!   member of a group of two or more and `push` from 2.55.0 against each of any group
//!   (S13), its refusal naming the key and `git dupe config --unset`; then the words;
//!   then, given no repository, the default remote.
//!   `remote` is not, so that `git dupe remote remove` can always repair such a remote.
//! - The guard stands wherever such a command's run could follow (`passthrough`): in an
//!   attached workspace before the private run, and before the help request's run where
//!   no workspace is attached, a locate that failed inside a Git directory included.
//!   Outside any repository there is no public place, and nothing is compared. The
//!   private configuration is asked only where the Git directory the run names is a
//!   directory: anywhere else there is no private remote and no default for the run to
//!   reach, and it can transfer nothing.
//! - Every run is made before anything is compared: the two of the places, then the
//!   private `config -z --list`, then, given no repository, or for a `push` or `pull`
//!   given a repository word and no refspec and no help request among the words, the
//!   private `symbolic-ref -q HEAD`, which the second reads for a hint alone, so that its
//!   failure there gives none and ends nothing. Their number follows the command word,
//!   the words, and where it stands, never what a repository or its configuration holds
//!   (G22, R4). Each is git-dupe's own and only reads (G5, R2); the `config` run has
//!   `GIT_CONFIG` removed and sees a remote a `-c` before `dupe` or an alias chain's
//!   prefix defines, which `Run` carries.
//! - A run the guard let through may end with one `hint:`, its line in `hints`, where
//!   Git's own advice names a plain `git` command, which under `git dupe` would act on the
//!   project's repository (`Holds/G18`, G25), the first that holds deciding: after a
//!   `push` given no repository whose configuration chooses no remote
//!   (`guards::transfer`), which runs as Git itself all the same, and that did not exit 0,
//!   naming `git dupe remote add`; after a `push` on a branch with no upstream that exited
//!   128, given no repository, or given the branch's remote and no refspec
//!   (`guards::transfer`), naming `git dupe push -u` with the remote Git takes or the word
//!   typed, and `HEAD`, which pushes the branch to the one of its name there whatever tag
//!   or other ref shares the name, where the branch's own name would be refused as
//!   ambiguous (S13), and, on a branch with no commit yet, where that push has no source
//!   to push, `git dupe commit` before it; after a `push` so given that exited 128 on a
//!   branch whose upstream has another name, under `push.default` unset or `simple`,
//!   naming `git dupe push <remote> HEAD:<upstream>` and `git dupe push <remote> HEAD`,
//!   the remote the branch's own, whose upstream the first pushes to; after a `push` given
//!   no repository, or a repository word and no refspec, that exited 128 on a detached
//!   `HEAD`, naming `git dupe push <remote> HEAD:<branch>` with the remote Git takes or
//!   the word typed; after any other `push` that exited 1, a rejected update or a hook's
//!   refusal alike, naming `git dupe pull` and `git dupe fetch`, given with the repository
//!   word typed, where one is, so that they name the destination that refused it and not
//!   the branch's upstream; after a `pull` that exited 1 on a branch with no upstream,
//!   which lacks `branch.<name>.merge` or `branch.<name>.remote`, given no repository, or
//!   given the branch's `branch.<name>.remote` and no refspec, naming `git dupe pull
//!   <remote> <branch>` and `git dupe branch --set-upstream-to`; after one that exited 128
//!   while neither the configuration nor an exact word of the six chooses how to reconcile
//!   divergent branches, naming `git dupe config pull.rebase` and `pull.ff`; after one
//!   that exited 1 on a detached `HEAD`, given no repository, or any repository word and no
//!   refspec, naming `git dupe pull <remote> <branch>`; after one that exited 128 where
//!   `--ff-only` is typed, or `pull.ff` is `only` and none of the six is, naming `git dupe
//!   merge --no-ff` and `git dupe rebase` of `FETCH_HEAD`; and after one that exited 0
//!   whose autostash came back with conflicts, `advice`'s line for it. Of the `pull` hints,
//!   the first that holds in that order follows, and the exits keep them apart: Git's
//!   advice for no branch to merge comes with exit 1, and a pull given a URL or a path has
//!   the remote's `HEAD` to merge, Git's advice where it cannot being the divergent-branches
//!   or fast-forward-only one, with 128. A
//!   bundle or an abbreviation of the six is not read, so the divergent-branches hint may
//!   follow where Git chose, never where a key did, and a `--ff-only` is read wherever it
//!   stands, the value of another option included; the detached and fast-forward lines
//!   say under which cause their commands do what they say.
//! - Each hint is decided from the records the guard already read and the run's end,
//!   never from what Git printed (R5), so it costs no run (G22) but two, each `advice`'s
//!   question, an answer of neither giving no hint: the private `rev-parse -q --verify
//!   HEAD` after a `push` that exited 128 owed the no-upstream hint, whose answer alone
//!   tells a branch with no commit, exit 1, from one with commits, exit 0 (S12); and
//!   whether the private index holds an unmerged entry, after a `pull` that exited 0,
//!   which leaves one only where its autostash came back with conflicts (S12). For a `pull` given a
//!   repository, the `branch.<name>.rebase` that keeps the divergent-branches hint away is
//!   any branch's, whether or not `HEAD` was read for the tracking hint.
//!   None follows a run among whose words is `-h` or `--help`, so that a help request's
//!   output stays Git's alone under every release, nor `fetch`, `remote`, or a run the
//!   guard did not let start; and none where the configuration was not asked, where
//!   there is no private repository.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use super::advice::{self, Answer};
use super::commands::{self, Command};
use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::places;
use crate::guards::places::Place;
use crate::guards::quoted::shell_word;
use crate::guards::transfer::{self, Current, DefaultRemote, Record, Repository, Transfer};
use crate::runner::locate::Facts;
use crate::runner::records;
use crate::runner::{Against, End, Run};
use hints::{
    DETACHED_PULL, FAST_FORWARD_ONLY, NO_REMOTE_CHOSEN, NOT_TRACKING, UNRECONCILED, detached,
    no_upstream, other_name, rejected, unborn,
};

mod hints;

/// The words of `pull` that choose how to reconcile, each matched exactly; `--rebase`
/// also with its value after `=`.
const RECONCILING: [&[u8]; 6] = [
    b"--rebase",
    b"-r",
    b"--no-rebase",
    b"--ff",
    b"--no-ff",
    b"--ff-only",
];

/// What follows the run of words the guard let through, with no help request among them
/// and the private configuration asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Then {
    /// Nothing of git-dupe's.
    Nothing,
    /// `push` given no repository whose configuration chooses no remote: its hint after a
    /// run that did not exit 0.
    NoRemoteChosen,
    /// Any other `push`: after a run that exited 128, the hint for what Git refused it
    /// for, where `refused` holds that; after a run that exited 1, the hint after a
    /// rejected update, naming the repository word `given`, where one was.
    Push {
        refused: Option<Refused>,
        given: Option<Vec<u8>>,
    },
    /// `pull`: after a run that exited 1, the hint for what left Git no branch to merge,
    /// where `unnamed` holds that; after one that exited 128, the hint for divergent
    /// branches, where `diverging` holds how Git was told to meet them; in the order of a
    /// branch with no upstream, an unchosen way, a detached `HEAD`, and fast-forward only;
    /// after a run that exited 0, the hint after an autostash that came back with
    /// conflicts, where the index then holds an unmerged entry.
    Pull {
        unnamed: Option<Unnamed>,
        diverging: Option<Diverging>,
    },
}

/// What leaves a `pull` given no refspec no branch to merge, where Git's advice names plain
/// `git pull <remote> <branch>` (S13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unnamed {
    /// A branch with no upstream, given no repository or the branch's remote.
    NoUpstream,
    /// A detached `HEAD`, given no repository or any repository word.
    Detached,
}

/// How a `pull` was told to meet divergent branches, where Git's advice after a run that
/// exited 128 names a plain command (S13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diverging {
    /// Not told: neither the configuration nor a word of the six chooses.
    Unchosen,
    /// Told to fast-forward only: `--ff-only` typed, or `pull.ff` `only` and none of the
    /// six typed, any of which overrides the key.
    FastForwardOnly,
}

/// What Git refuses a `push` given no refspec for, exit 128, naming a plain command
/// (S13), with the remote the hint names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A branch with no upstream: the remote Git takes, given no repository, or the
    /// branch's remote typed.
    NoUpstream { remote: Vec<u8> },
    /// A branch whose upstream has another name: the branch's remote, and the raw value
    /// of its `merge`.
    OtherName { remote: Vec<u8>, merge: Vec<u8> },
    /// A detached `HEAD`: the remote Git takes, given no repository, or the word typed.
    Detached { remote: Vec<u8> },
}

/// Whether `words`, the command word first, may run against the repository `against`
/// names where `facts` locate the project, and what follows its run: `Err` with the
/// refusal of the first thing that names a public place, or with the end of a run the
/// guard needed.
pub fn guard(words: &[OsString], facts: &Facts, against: Against) -> Result<Then, Outcome> {
    let (command, after) = words
        .split_first()
        .expect("a guarded command's words begin with its word");
    let after: Vec<&[u8]> = after.iter().map(|word| word.as_bytes()).collect();
    let moving = match commands::named(command.as_bytes()) {
        Some(Command::Push) => Some(Transfer::Push),
        Some(Command::Pull) => Some(Transfer::Pull),
        Some(Command::Fetch) => Some(Transfer::Fetch),
        _ => None,
    };
    let places = places::take(facts)?;
    let private = moving.filter(|_| against.private_repository().is_some());
    let repository = private.map(|moving| transfer::repository(moving, &after));
    let configuration = match private {
        Some(_) => places::asked(Run::against(against, ["config", "-z", "--list"]), false)?,
        None => Vec::new(),
    };
    let help = after.iter().any(|word| matches!(*word, b"-h" | b"--help"));
    let head = match (private, repository) {
        (_, Some(Repository::Default)) => Some(current_branch(against)?),
        // Read for a hint alone, which no help request draws.
        (Some(Transfer::Push | Transfer::Pull), Some(Repository::Alone(_))) if !help => {
            current_branch(against).ok()
        }
        _ => None,
    };

    let records: Vec<transfer::Record> = records::config_records(&configuration).collect();
    for (name, url) in transfer::remote_urls(&records) {
        if let Some(place) = places.named_by_whole(url) {
            return Err(refuse_remote(name, place));
        }
    }
    for (key, member) in transfer::group_members(&records) {
        if let Some(place) = places.named_by(member) {
            return Err(refuse_group(key, place));
        }
    }
    for word in &after {
        if let Some(place) = places.named_by(word) {
            return Err(places::refuse(word, place));
        }
    }
    if let (Some(moving), Some(Repository::Default), Some(head)) = (private, repository, &head) {
        let found = transfer::default_remote(moving, &records, records::branch_of(head));
        if let Some(found) = found
            && let Some(place) = places.named_by_whole(found.value)
        {
            return Err(refuse_default(command.as_bytes(), found, place));
        }
    }
    let current = match &head {
        None => Current::AnyBranch,
        Some(head) => records::branch_of(head).map_or(Current::Detached, Current::Branch),
    };
    let chooses_no_remote = match current {
        Current::Branch(branch) => transfer::push_chooses_no_remote(&records, Some(branch)),
        Current::Detached => transfer::push_chooses_no_remote(&records, None),
        // Given no repository, `HEAD` is always read.
        Current::AnyBranch => false,
    };
    Ok(match (private, repository) {
        _ if help => Then::Nothing,
        (Some(Transfer::Push), Some(Repository::Default)) if chooses_no_remote => {
            Then::NoRemoteChosen
        }
        (Some(Transfer::Push), Some(given)) => Then::Push {
            refused: refused(&records, given, current),
            given: given.word().map(<[u8]>::to_vec),
        },
        (Some(Transfer::Pull), Some(given)) => Then::Pull {
            unnamed: unnamed(&records, given, current),
            diverging: diverging(&records, given, current, &after),
        },
        _ => Then::Nothing,
    })
}

/// What Git would refuse `push` for, given `given` with `HEAD` at `current`, where its
/// advice names a plain command and the configuration's `records` choose a remote: none
/// given a refspec, nor on a branch given a repository word other than the branch's
/// remote, to which Git pushes the branch under its own name (S13). Where several would
/// follow, the first holds: a branch has no upstream, or one of another name.
fn refused(records: &[Record<'_>], given: Repository<'_>, current: Current<'_>) -> Option<Refused> {
    let word = match given {
        Repository::WithRefspec(_) => return None,
        Repository::Default | Repository::Alone(_) => given.word(),
    };
    let branch = match current {
        Current::Branch(branch) => branch,
        Current::Detached => {
            let remote = word.unwrap_or_else(|| transfer::push_remote(records, None));
            return Some(Refused::Detached {
                remote: remote.to_vec(),
            });
        }
        Current::AnyBranch => return None,
    };
    if let Some(word) = word
        && !transfer::is_the_branch_remote(Transfer::Push, records, branch, word)
    {
        return None;
    }
    if !transfer::has_upstream(records, branch) {
        let remote = word.unwrap_or_else(|| transfer::push_remote(records, Some(branch)));
        return Some(Refused::NoUpstream {
            remote: remote.to_vec(),
        });
    }
    let upstream = transfer::upstream_of_another_name(records, branch)?;
    transfer::push_default_is_simple(records).then(|| Refused::OtherName {
        remote: upstream.remote.to_vec(),
        merge: upstream.merge.to_vec(),
    })
}

/// What leaves `pull`, given `given` with `HEAD` at `current`, no branch to merge, where
/// Git's advice names `git pull <remote> <branch>` and the configuration's `records` hold
/// what Git read (S13): nothing given a refspec; on a branch, no upstream, either branch
/// key unset, given no repository or the branch's own `branch.<name>.remote`; a detached
/// `HEAD`, whatever repository word is given, because no branch key names one for it.
fn unnamed(records: &[Record<'_>], given: Repository<'_>, current: Current<'_>) -> Option<Unnamed> {
    let branch = match (given, current) {
        (Repository::WithRefspec(_), _) | (_, Current::AnyBranch) => return None,
        (_, Current::Detached) => return Some(Unnamed::Detached),
        (_, Current::Branch(branch)) => branch,
    };
    let the_branch_remote = match given {
        Repository::Alone(word) => {
            transfer::is_the_branch_remote(Transfer::Pull, records, branch, word)
        }
        Repository::Default | Repository::WithRefspec(_) => true,
    };
    (the_branch_remote && !transfer::has_upstream(records, branch)).then_some(Unnamed::NoUpstream)
}

/// How `pull`, given `given` with `HEAD` at `current` and the words `after` its command
/// word, was told to meet divergent branches, where the configuration's `records` hold
/// what Git read (S13): a `--ff-only` typed tells it to fast-forward only; otherwise a word
/// of the six chooses, whatever the configuration says; otherwise `pull.ff` set to `only`
/// tells it to; otherwise any reconciliation configured chooses, the `branch.<name>.rebase`
/// of any branch where a repository is given.
fn diverging(
    records: &[Record<'_>],
    given: Repository<'_>,
    current: Current<'_>,
    after: &[&[u8]],
) -> Option<Diverging> {
    if after.iter().any(|&word| word == b"--ff-only") {
        return Some(Diverging::FastForwardOnly);
    }
    if after.iter().any(|word| reconciling(word)) {
        return None;
    }
    if transfer::pull_ff_is_only(records) {
        return Some(Diverging::FastForwardOnly);
    }
    let current = match given {
        Repository::Default => current,
        Repository::Alone(_) | Repository::WithRefspec(_) => Current::AnyBranch,
    };
    (!transfer::reconciliation_configured(records, current)).then_some(Diverging::Unchosen)
}

/// Whether `word` is one of the words of `pull` that choose how to reconcile.
fn reconciling(word: &[u8]) -> bool {
    RECONCILING.contains(&word) || word.starts_with(b"--rebase=")
}

/// A question of the private repository's state that a hint puts after the run, at most
/// one per run (G22).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Question {
    /// Whether `HEAD` names a commit.
    HeadVerifies,
    /// Whether the index holds an unmerged entry.
    Unmerged,
}

/// Writes what follows a run that ended as `ended` against the repository `against`
/// names, the guard having decided `then`.
pub fn after(then: Then, ended: &Outcome, against: Against) {
    let ask = |question| match question {
        Question::HeadVerifies => advice::verifies("HEAD", against),
        Question::Unmerged => advice::unmerged(against),
    };
    if let Some(hint) = hint(&then, ended, ask) {
        lines::write(Level::Hint, &hint);
    }
}

/// The hint that follows a run that started and ended as `ended`, the guard having
/// decided `then`: "did not exit 0" is any end but exit 0, a signal included, and an end
/// named by its number is never a signal's. A run that did not start is no run. `ask`
/// is put a question only where the hint it decides is due: whether `HEAD` names a commit
/// after a `push` that exited 128 owed the no-upstream hint, and whether the index holds
/// an unmerged entry after a `pull` that exited 0 (G22).
fn hint(then: &Then, ended: &Outcome, ask: impl FnOnce(Question) -> Answer) -> Option<Vec<u8>> {
    let Outcome::Git(end) = ended else {
        return None;
    };
    let failed = *end != End::Code(0);
    match then {
        Then::NoRemoteChosen if failed => Some(NO_REMOTE_CHOSEN.to_vec()),
        Then::Push {
            refused: Some(refused),
            ..
        } if *end == End::Code(128) => refusal(refused, || ask(Question::HeadVerifies)),
        Then::Push { given, .. } if *end == End::Code(1) => Some(rejected(given.as_deref())),
        // In the table's order, the first that holds (`Holds/G18`): Git's advice for no
        // branch to merge comes with exit 1, and for divergent branches with 128.
        Then::Pull {
            unnamed: Some(Unnamed::NoUpstream),
            ..
        } if *end == End::Code(1) => Some(NOT_TRACKING.to_vec()),
        Then::Pull {
            diverging: Some(Diverging::Unchosen),
            ..
        } if *end == End::Code(128) => Some(UNRECONCILED.to_vec()),
        Then::Pull {
            unnamed: Some(Unnamed::Detached),
            ..
        } if *end == End::Code(1) => Some(DETACHED_PULL.to_vec()),
        Then::Pull {
            diverging: Some(Diverging::FastForwardOnly),
            ..
        } if *end == End::Code(128) => Some(FAST_FORWARD_ONLY.to_vec()),
        Then::Pull { .. } if *end == End::Code(0) => {
            (ask(Question::Unmerged) == Answer::Yes).then(|| advice::AUTOSTASH.to_vec())
        }
        _ => None,
    }
}

/// The hint after a `push` that Git refused, exit 128, for `refused`. A branch with no
/// upstream is asked whether `HEAD` names a commit: where it does not, the branch is
/// unborn, and the push the hint names would have no source to push (S13).
fn refusal(refused: &Refused, head_verifies: impl FnOnce() -> Answer) -> Option<Vec<u8>> {
    match refused {
        Refused::NoUpstream { remote } => match head_verifies() {
            Answer::Yes => Some(no_upstream(remote)),
            Answer::No => Some(unborn(remote)),
            Answer::Unanswered => None,
        },
        Refused::OtherName { remote, merge } => Some(other_name(remote, merge)),
        Refused::Detached { remote } => Some(detached(remote)),
    }
}

/// The answer of the private `symbolic-ref -q HEAD`: its line on a branch, nothing on a
/// detached `HEAD`, which exits 1 (S12). An empty answer on a branch cannot be read.
fn current_branch(against: Against) -> Result<Vec<u8>, Outcome> {
    let run = Run::against(against, ["symbolic-ref", "-q", "HEAD"]).capture_output();
    let finished = run.start().map_err(outcome::not_started)?;
    match finished.end {
        End::Code(0) if finished.stdout.is_empty() => Err(outcome::unreadable()),
        End::Code(0) => Ok(finished.stdout),
        End::Code(1) => Ok(Vec::new()),
        end => Err(Outcome::Git(end)),
    }
}

/// The refusal of the private remote `name`, a URL of which names `place`.
fn refuse_remote(name: &[u8], place: &Place) -> Outcome {
    let mut line = b"a URL of the private remote ".to_vec();
    line.extend_from_slice(&lines::typed(name));
    line.extend_from_slice(b" names ");
    line.extend_from_slice(&places::described(place));
    line.extend_from_slice(b"; 'git dupe remote remove' removes that remote");
    outcome::refuse(&line)
}

/// The refusal of the private remote group `key`, a member of which names `place`. The
/// member is not named: it is the configuration's, a URL that may hold a credential (G25).
fn refuse_group(key: &[u8], place: &Place) -> Outcome {
    let mut line = b"a member of the private remote group ".to_vec();
    line.extend_from_slice(&lines::typed(key));
    line.extend_from_slice(b" names ");
    line.extend_from_slice(&places::described(place));
    line.extend_from_slice(b"; 'git dupe config --unset ");
    line.extend_from_slice(&shell_word(key));
    line.extend_from_slice(b"' removes that group");
    outcome::refuse(&line)
}

/// The refusal of `command` given no repository, whose default remote names `place`.
fn refuse_default(command: &[u8], found: DefaultRemote, place: &Place) -> Outcome {
    let mut line = command.to_vec();
    line.extend_from_slice(b" given no repository takes ");
    line.extend_from_slice(&lines::typed(found.value));
    line.extend_from_slice(b" from ");
    line.extend_from_slice(found.key);
    line.extend_from_slice(b", which names ");
    line.extend_from_slice(&places::described(place));
    line.extend_from_slice(b"; 'git dupe git' runs Git unguarded, for a destination you mean");
    outcome::refuse(&line)
}

#[cfg(test)]
mod tests;
