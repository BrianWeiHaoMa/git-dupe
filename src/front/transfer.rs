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
//!   dupe remote remove`; then the words; then, given no repository, the default remote.
//!   `remote` is not, so that `git dupe remote remove` can always repair such a remote.
//! - The guard stands wherever such a command's run could follow (`passthrough`): in an
//!   attached workspace before the private run, and before the help request's run where
//!   no workspace is attached, a locate that failed inside a Git directory included.
//!   Outside any repository there is no public place, and nothing is compared. The
//!   private configuration is asked only where the Git directory the run names is a
//!   directory: anywhere else there is no private remote and no default for the run to
//!   reach, and it can transfer nothing.
//! - Every run is made before anything is compared: the two of the places, then the
//!   private `config -z --list`, then, given no repository, the private `symbolic-ref -q
//!   HEAD`. Their number follows the command word, the words, and where it stands, never
//!   what a repository or its configuration holds (G22, R4). Each is git-dupe's own and
//!   only reads (G5, R2); the `config` run has `GIT_CONFIG` removed and sees a remote a
//!   `-c` before `dupe` or an alias chain's prefix defines, which `Run` carries.
//! - A run the guard let through may end with one `hint:` where Git's own advice names a
//!   plain `git` command, which under `git dupe` would act on the project's repository
//!   (`Holds/G18`, G25): after a `push` given no repository whose configuration chooses
//!   no remote (`guards::transfer`), which runs as Git itself all the same, and that did
//!   not exit 0, naming `git dupe remote add`; after a `push` given no repository, on a
//!   branch with no upstream, that exited 128, naming `git dupe push -u` with the remote
//!   Git takes and `HEAD`, which pushes the branch to the one of its name there whatever
//!   tag or other ref shares the name, where the branch's own name would be refused as
//!   ambiguous (S13); after any other `push` that exited 1, a rejected update
//!   or a hook's refusal alike, naming `git dupe pull` and `git dupe fetch`; after a
//!   `pull` given no repository, on a branch with no upstream, that did not exit 0,
//!   naming `git dupe pull <remote> <branch>` and `git dupe branch --set-upstream-to`;
//!   and after any other `pull` that exited 128 while neither the configuration nor an
//!   exact word of the six chooses how to reconcile divergent branches, naming
//!   `git dupe config pull.rebase` and `pull.ff`. A bundle or an abbreviation of those
//!   words is not read, so the last may follow where Git chose, never where a key did.
//! - Each hint is decided from the records the guard already read and the run's end
//!   alone, never from what Git printed (R5), so it costs no run (G22): a `pull` given a
//!   repository has no `HEAD` read, and its `branch.<name>.rebase` is then any branch's.
//!   None follows a run among whose words is `-h` or `--help`, so that a help request's
//!   output stays Git's alone under every release, nor `fetch`, `remote`, or a run the
//!   guard did not let start; and none where the configuration was not asked, where
//!   there is no private repository.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use super::commands::{self, Command};
use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::places;
use crate::guards::places::Place;
use crate::guards::quoted::shell_word;
use crate::guards::transfer::{self, Current, DefaultRemote, Record, Transfer};
use crate::runner::locate::Facts;
use crate::runner::records;
use crate::runner::{Against, End, Run};

/// The hint after a failed `push` whose configuration chooses no remote: true whichever
/// remote Git took, the only one of another name included, so it never says that none
/// exists.
const NO_REMOTE_CHOSEN: &[u8] = b"to push private history, 'git dupe remote add <name> <url>' \
    adds a private remote and 'git dupe push -u <name> <branch>' pushes to it; plain \
    'git remote' and 'git push' act on the project's repository";

/// The hint after a rejected `push`: Git's advice for a rejected update names plain
/// `git pull`. It says what to do in that case, true too after a hook's refusal.
const REJECTED: &[u8] = b"where a private push is rejected because the remote holds commits \
    this branch lacks, 'git dupe pull', or 'git dupe fetch' and then a merge or rebase, \
    brings them here before the next push; plain 'git pull' and 'git fetch' act on the \
    project's repository";

/// The hint after a failed `pull` on a branch with no upstream.
const NOT_TRACKING: &[u8] = b"the current private branch has no upstream to pull from: \
    'git dupe pull <remote> <branch>' pulls a branch this once, and \
    'git dupe branch --set-upstream-to=<remote>/<branch>' makes it the upstream; plain \
    'git pull' and 'git branch' act on the project's repository";

/// The hint after a `pull` that exited 128 with no way to reconcile chosen.
const UNRECONCILED: &[u8] = b"where a private pull finds divergent branches, \
    'git dupe config pull.rebase false' has it merge, 'git dupe config pull.rebase true' \
    rebase, and 'git dupe config pull.ff only' take only a fast-forward; plain \
    'git config' acts on the project's repository";

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
    /// Any other `push`: after a run that exited 128, the hint naming `git dupe push -u`,
    /// when given no repository on a branch with no upstream, which this holds; after a
    /// run that exited 1, the hint after a rejected update.
    Push(Option<Upstream>),
    /// `pull`: after a run that did not exit 0, the hint for a branch with no upstream,
    /// when `untracked`; otherwise, after a run that exited 128, the hint naming how to
    /// reconcile, when `unreconciled`.
    Pull { untracked: bool, unreconciled: bool },
}

/// The remote Git takes for a branch with no upstream that `push` given no repository
/// was asked to push.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upstream {
    remote: Vec<u8>,
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
    let configuration = match private {
        Some(_) => places::asked(Run::against(against, ["config", "-z", "--list"]), false)?,
        None => Vec::new(),
    };
    let head = match private {
        Some(moving) if !transfer::gives_a_repository(moving, &after) => {
            Some(current_branch(against)?)
        }
        _ => None,
    };

    let records: Vec<transfer::Record> = records::config_records(&configuration).collect();
    for (name, url) in transfer::remote_urls(&records) {
        if let Some(place) = places.named_by_whole(url) {
            return Err(refuse_remote(name, place));
        }
    }
    for word in &after {
        if let Some(place) = places.named_by(word) {
            return Err(places::refuse(word, place));
        }
    }
    if let (Some(moving), Some(head)) = (private, &head) {
        let found = transfer::default_remote(moving, &records, records::branch_of(head));
        if let Some(found) = found
            && let Some(place) = places.named_by_whole(found.value)
        {
            return Err(refuse_default(command.as_bytes(), found, place));
        }
    }
    let help = after.iter().any(|word| matches!(*word, b"-h" | b"--help"));
    let current = match &head {
        None => Current::Unread,
        Some(head) => records::branch_of(head).map_or(Current::Detached, Current::Branch),
    };
    Ok(match private {
        _ if help => Then::Nothing,
        Some(Transfer::Push) => match current {
            Current::Unread => Then::Push(None),
            Current::Detached if transfer::push_chooses_no_remote(&records, None) => {
                Then::NoRemoteChosen
            }
            Current::Detached => Then::Push(None),
            Current::Branch(branch) if transfer::push_chooses_no_remote(&records, Some(branch)) => {
                Then::NoRemoteChosen
            }
            Current::Branch(branch) => Then::Push(without_upstream(&records, branch)),
        },
        Some(Transfer::Pull) => Then::Pull {
            untracked: matches!(current, Current::Branch(branch)
                if !transfer::has_upstream(&records, branch)),
            unreconciled: !transfer::reconciliation_configured(&records, current)
                && !after.iter().any(|word| reconciling(word)),
        },
        Some(Transfer::Fetch) | None => Then::Nothing,
    })
}

/// The upstream `push` given no repository on `branch` would set, when the configuration's
/// `records` give the branch none.
fn without_upstream(records: &[Record<'_>], branch: &[u8]) -> Option<Upstream> {
    (!transfer::has_upstream(records, branch)).then(|| Upstream {
        remote: transfer::push_remote(records, Some(branch)).to_vec(),
    })
}

/// Whether `word` is one of the words of `pull` that choose how to reconcile.
fn reconciling(word: &[u8]) -> bool {
    RECONCILING.contains(&word) || word.starts_with(b"--rebase=")
}

/// Writes what follows a run that ended as `ended`, the guard having decided `then`.
pub fn after(then: Then, ended: &Outcome) {
    if let Some(hint) = hint(&then, ended) {
        lines::write(Level::Hint, &hint);
    }
}

/// The hint that follows a run that started and ended as `ended`, the guard having
/// decided `then`: "did not exit 0" is any end but exit 0, a signal included, and an end
/// named by its number is never a signal's. A run that did not start is no run.
fn hint(then: &Then, ended: &Outcome) -> Option<Vec<u8>> {
    let Outcome::Git(end) = ended else {
        return None;
    };
    let failed = *end != End::Code(0);
    match then {
        Then::NoRemoteChosen if failed => Some(NO_REMOTE_CHOSEN.to_vec()),
        Then::Push(Some(upstream)) if *end == End::Code(128) => Some(no_upstream(upstream)),
        Then::Push(_) if *end == End::Code(1) => Some(REJECTED.to_vec()),
        Then::Pull {
            untracked: true, ..
        } if failed => Some(NOT_TRACKING.to_vec()),
        Then::Pull {
            unreconciled: true, ..
        } if *end == End::Code(128) => Some(UNRECONCILED.to_vec()),
        _ => None,
    }
}

/// The hint after a `push` that exited 128 on a branch with no upstream, naming the
/// remote Git takes and `HEAD` for the branch, whose own name a tag or another ref may
/// share.
fn no_upstream(upstream: &Upstream) -> Vec<u8> {
    [
        &b"the current private branch has no upstream: 'git dupe push -u "[..],
        &offered_remote(&upstream.remote),
        b" HEAD' pushes it and sets one; plain 'git push' acts on the project's repository",
    ]
    .concat()
}

/// The remote's name or URL `name` as a shell reads it back as one word, or the
/// placeholder `<remote>` where none is to be named: a value from the configuration may
/// be empty, begin with `-`, which `push` reads as an option, or hold a control byte,
/// which would break the line (F8).
fn offered_remote(name: &[u8]) -> Vec<u8> {
    let named =
        name.first().is_some_and(|&first| first != b'-') && !name.iter().any(u8::is_ascii_control);
    if named {
        shell_word(name)
    } else {
        b"<remote>".to_vec()
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
