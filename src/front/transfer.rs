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
//! - `push` given no repository whose configuration chooses no remote
//!   (`guards::transfer`) runs as Git itself all the same, and a run that did not exit 0
//!   ends with one `hint:`, because Git's own advice there names plain `git remote add`
//!   or `git push`, which under `git dupe` would act on the project's repository. It is
//!   decided from the records the guard already read and the run's end alone, never from
//!   what Git printed (R5), so it costs no run (G22). None follows a `push` among whose
//!   words is `-h` or `--help`, so that a help request's output stays Git's alone under
//!   every release, nor `pull`, `fetch`, `remote`, or a run the guard did not let start;
//!   and none where the configuration was not asked, where there is no private
//!   repository to add a remote to.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use super::commands::{self, Command};
use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use super::places;
use crate::guards::places::Place;
use crate::guards::transfer::{self, DefaultRemote, Transfer};
use crate::runner::locate::Facts;
use crate::runner::records;
use crate::runner::{Against, End, Run};

/// The hint after a failed `push` whose configuration chooses no remote: true whichever
/// remote Git took, the only one of another name included, so it never says that none
/// exists.
const NO_REMOTE_CHOSEN: &[u8] = b"to push private history, 'git dupe remote add <name> <url>' \
    adds a private remote and 'git dupe push -u <name> <branch>' pushes to it; plain \
    'git remote' and 'git push' act on the project's repository";

/// What follows the run of words the guard let through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    /// Nothing of git-dupe's.
    Nothing,
    /// The hint, after a run that did not exit 0: `push` given no repository whose
    /// configuration chooses no remote, with no help request among its words.
    HintUnlessSucceeded,
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
    let private = moving.filter(|_| names_a_directory(against));
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
    Ok(match (private, &head) {
        (Some(Transfer::Push), Some(head))
            if !help && transfer::push_chooses_no_remote(&records, records::branch_of(head)) =>
        {
            Then::HintUnlessSucceeded
        }
        _ => Then::Nothing,
    })
}

/// Writes what follows a run that ended as `ended`, the guard having decided `then`.
pub fn after(then: Then, ended: &Outcome) {
    if hinted(then, ended) {
        lines::write(Level::Hint, NO_REMOTE_CHOSEN);
    }
}

/// Whether the hint follows: after a run that started and did not exit 0, a run a signal
/// killed included. A run that did not start is no run.
fn hinted(then: Then, ended: &Outcome) -> bool {
    then == Then::HintUnlessSucceeded && matches!(ended, Outcome::Git(end) if *end != End::Code(0))
}

/// Whether the Git directory `against` names is a directory: a private repository that
/// can hold remotes, a default, and a branch.
fn names_a_directory(against: Against) -> bool {
    match against {
        Against::Private { git_directory, .. }
        | Against::PrivateWithoutWorkTree { git_directory } => git_directory.is_dir(),
        Against::Public => false,
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
mod tests {
    use super::*;

    #[test]
    fn the_hint_follows_a_run_that_did_not_exit_0() {
        let then = Then::HintUnlessSucceeded;
        assert!(!hinted(then, &Outcome::Git(End::Code(0))));
        assert!(hinted(then, &Outcome::Git(End::Code(128))));
        assert!(hinted(then, &Outcome::Git(End::Code(1))));
        assert!(hinted(then, &Outcome::Git(End::Signal(9))));
        // A run that did not start, or a refusal, is no run.
        assert!(!hinted(then, &Outcome::Refused));
        for ended in [
            Outcome::Git(End::Code(0)),
            Outcome::Git(End::Code(128)),
            Outcome::Git(End::Signal(9)),
        ] {
            assert!(!hinted(Then::Nothing, &ended));
        }
    }

    #[test]
    fn the_hint_names_the_git_dupe_commands_and_never_says_no_remote_exists() {
        let hint = String::from_utf8(NO_REMOTE_CHOSEN.to_vec()).unwrap();
        assert!(
            hint.contains("'git dupe remote add <name> <url>'"),
            "{hint}"
        );
        assert!(
            hint.contains("'git dupe push -u <name> <branch>'"),
            "{hint}"
        );
        assert!(
            hint.find("git dupe remote add") < hint.find("git dupe push -u"),
            "{hint}"
        );
        assert!(!hint.contains("no remote"), "{hint}");
        assert!(!hint.contains('\n'), "{hint}");
    }
}
