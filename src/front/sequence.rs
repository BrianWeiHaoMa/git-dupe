//! The sequence every command runs, its steps numbered as `Composition/Front` of the
//! technical specification numbers them: (1) through (9), step (2) reading the words of
//! every command F6 names: `help`, `init`, `clone`, `detach`, `hide`, `unhide`, `status`,
//! `add`, `clean`, `git`, `stash`, `push`, `pull`, `fetch`, and `remote`.
//!
//! The order is the guard. What needs no repository is answered before the locate run;
//! a misuse is reported after it, so that outside a repository Git's own message ends the
//! command, and in an unattached one a command that requires attachment is refused naming
//! `git dupe init` first. A word F6 does not name is resolved in step (4), after the
//! locate, because its aliases are read where its run would be made (`alias`); the command
//! a chain reaches is read as step (2) reads it typed, and goes on as that command. Every
//! command that gets past step (5) but `detach` is one G6 covers, and ends with the
//! keeper's settle in step (8) whenever the workspace is attached after its handler,
//! whatever the handler returned, a command Git ran as itself included: settle lives here
//! and in no handler (R3), and so follows a `clone` wherever it ends attached. `detach`,
//! typed or reached through an alias, leaves the sequence before step (6), so that it
//! neither starts from the region nor settles, whatever it returns. Where no workspace is
//! attached, a help request after a word passed through, after `stash`, or after `push`,
//! `pull`, `fetch`, or `remote`, and, of a command an alias reaches, a help request or a
//! usage error step (2) answers at once (a word a table does not hold, a Git global option
//! after `git`), are answered alone, and nothing settles; a misuse held of such a command
//! ends as a typed one does, with Git's message or step (5)'s refusal (`Holds/G24`,
//! `Holds/G4`). Each of the three sites that runs a command Git runs as itself — step (5),
//! step (7), and `answered_unlocated` — hands `Passed::run` the facts of where the command
//! stands and the repository its run is made against, so that the transfer commands'
//! guard compares at every one of them (`Holds/G18`).
//!
//! The file is full at one concern, the sequence and the wiring of every command: each step
//! with a body is a function here, which calls the sibling module that owns what the step
//! asks (`alias`, a command's own module, the keeper); a new concern goes in a sibling file.
//!
//! A new command is wired in at these sites, each a plain line that search finds:
//!
//! - `src/front/<word>.rs`: the reading of its words, its handler, and the text of its
//!   own lines; its `mod` line in `src/front.rs`. A command with a table keeps the table
//!   there and reads its words through `with_table` alone, as `status`, `add`, and
//!   `clean` do: in step (2) a help request and a word the table does not hold are
//!   answered at once. Where its operands are literal paths, as `status`'s and `add`'s
//!   are, one that is no literal path is held as a misuse there, and in `handler` one
//!   outside the working tree is the misuse; `clean`'s are Git's own pathspecs and are
//!   neither (`Holds/G16`).
//! - Here: one `Read` and one `Handler` variant, and an arm in `read_words` (step (2)),
//!   in `handler`, and in step (7).
//! - `src/front/commands.rs`: the word in `WORDS` and its `Command`.
//! - `src/front/help/<word>.txt`; in `src/front/help.rs` its `include_str!` constant, its
//!   `COMMANDS` entry, and its word in the check
//!   `every_command_that_reads_its_own_words_has_a_text`; its sentences in
//!   `src/front/help/general.txt`, and, where a day's use needs it, in the manual page's own
//!   texts under `src/front/page/`, whose command lines a scenario runs. `build.rs` reads
//!   both directories: no edit there.
//! - `tests/scenarios/<word>.rs` and its `mod` line in `tests/scenarios/main.rs`.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

use super::add;
use super::alias::{self, Resolved};
use super::clean;
use super::clone;
use super::commands::{self, Command};
use super::detach;
use super::git;
use super::help;
use super::hide;
use super::init;
use super::lines;
use super::lines::Level;
use super::outcome::{self, Outcome};
use super::passthrough::Passed;
use super::status;
use super::table::{self, Entry, Fault, Reading};
use crate::keeper::{self, StartingRegion};
use crate::runner::locate::{self, NotLocated, Workspace};
use crate::runner::{self, Against};

const VERSION_LINE: &str = concat!("git-dupe version ", env!("CARGO_PKG_VERSION"), "\n");

/// The usage error of a dashed first word other than `-h` alone and `--version` (F7).
const DASHED_FIRST_WORD: &[u8] = b"after 'dupe' only -h alone or --version may begin with a dash; \
    Git's own options go before it: git <options> dupe <command>";

/// Runs the words after `dupe` through the sequence and returns the exit status.
pub fn run(words: &[OsString]) -> u8 {
    // (9) The exit status: the one place an outcome becomes one.
    sequence(words).status()
}

fn sequence(words: &[OsString]) -> Outcome {
    // (1) Bare `git dupe`, `--version`, `-h` alone, and every other dashed first word.
    let first = match first_word(words) {
        Ok(first) => first,
        Err(answered) => return answered,
    };

    // (2) The words of an own, changed, or guarded command, read before locating; a misuse
    // is held. Any other word waits for step (4).
    let literal = commands::named(first);
    let read = match literal.map(|command| read_words(command, words)) {
        Some(Words::Answered(answered)) => return answered,
        Some(Words::Read(read)) => Some(read),
        None => None,
    };

    // (3) The runner locates the workspace; its failure ends the command as Git ended it,
    // but for what `answered_unlocated` answers where the failed locate leaves it.
    let workspace = match locate::locate(literal.is_some_and(Command::attaches)) {
        Ok(workspace) => workspace,
        Err(not_located) => {
            return answered_unlocated(read, words, &not_located)
                .unwrap_or_else(|| not_located_end(not_located));
        }
    };

    // (4) A first word F6 does not name is resolved through the aliases where its run
    // would be made (`through_aliases`); a lookup Git failed ends the command, settled
    // where attached (R3), and a command that attaches, so reached, is located again
    // (`relocated`).
    let mut expanded = None;
    let (command, read) = match read {
        Some(read) => (literal, read),
        None => match aliased(&workspace, words, &mut expanded) {
            Reached::Read(command, read) => (command, read),
            Reached::Answered(answered) => return answered,
            Reached::Failed(ended) => return ended_early(&workspace, ended),
        },
    };
    let attaching = command.is_some_and(Command::attaches);
    let workspace = match relocated(workspace, attaching && literal.is_none()) {
        Ok(workspace) => workspace,
        Err(ended) => return ended,
    };

    // (5) Every command but the ones that attach requires this worktree to be attached,
    // whatever the other worktrees hold (`unattached`).
    if !workspace.attached() && !attaching {
        return unattached(&workspace, &read);
    }

    // A misuse held from (2) or (4), then an operand that names the root or a path outside
    // it, which only the user's directory below the root reveals: no settle follows a
    // usage error.
    let handler = match read.and_then(|read| handler(read, &workspace, command)) {
        Ok(handler) => handler,
        Err(fault) => return misused(&fault, command),
    };

    // `detach` is no command G6 covers: it ends here, without the starting region of step
    // (6) and the settle of step (8), so that a refusal changes nothing and nothing writes
    // the region back once it is deleted (R3).
    if let Handler::Detach(force) = handler {
        return detach::run(&workspace, force);
    }

    // (6) The region as the command begins, in a workspace attached before it.
    let starting = if workspace.attached() {
        keeper::start(&workspace)
    } else {
        StartingRegion::default()
    };

    // (7) The handler, then (8) settle, in a workspace attached after it.
    let handled = handle(handler, &workspace);
    settle(&workspace, &starting, handled)
}

/// Step (1): the first word, or the outcome the command ends with before anything runs:
/// bare `git dupe`, `--version`, `-h` alone, and every other dashed first word.
fn first_word(words: &[OsString]) -> Result<&[u8], Outcome> {
    let usage_line = help::general_usage_line();
    let Some((first, rest)) = words.split_first() else {
        return Err(outcome::print(usage_line, Outcome::BareUsage));
    };
    let first = first.as_bytes();
    if first == b"--version" {
        return Err(outcome::print(VERSION_LINE.as_bytes(), Outcome::Answered));
    }
    if first == b"-h" && rest.is_empty() {
        return Err(help::general());
    }
    if first.starts_with(b"-") {
        // Git's global options, unknown dashed words, and `-h` with a word after it are
        // not told apart: nothing else dashed is read here.
        return Err(outcome::usage_error(DASHED_FIRST_WORD, usage_line));
    }
    Ok(first)
}

/// Step (2): the words of `command`, read before locating when typed, and in step (4)
/// when an alias reached it. `words` are all of them, the word that named the command
/// first; texts and lines name the command by its own word, so `stage` reads as `add`.
/// `help` needs no repository, and a help request is answered here, as is the usage error
/// F7 names for `git`. A misuse is held until after steps (3) and (5), so that outside a
/// repository Git's own message ends the command, and in an unattached one a command that
/// requires attachment is refused naming `git dupe init` first. The words of a command
/// Git runs as itself are not read here: its guard reads them when it runs.
fn read_words(command: Command, words: &[OsString]) -> Words<'_> {
    let word = command.word();
    let rest = &words[1..];
    match command {
        Command::Help => Words::Answered(help::run(rest)),
        Command::Init => match init::read(rest) {
            init::Asked::Help => Words::Answered(help::command(word)),
            init::Asked::Attach(branch) => Words::Read(Ok(Read::Init(branch))),
            init::Asked::Misused(fault) => Words::Read(Err(fault.to_vec())),
        },
        Command::Clone => match clone::read(rest) {
            clone::Asked::Help => Words::Answered(help::command(word)),
            clone::Asked::Clone { url, branch } => Words::Read(Ok(Read::Clone {
                words: rest,
                url,
                branch,
            })),
            clone::Asked::Misused(fault) => Words::Read(Err(fault.to_vec())),
        },
        Command::Detach => match detach::read(rest) {
            detach::Asked::Help => Words::Answered(help::command(word)),
            detach::Asked::Detach { force } => Words::Read(Ok(Read::Detach(force))),
            detach::Asked::Misused(fault) => Words::Read(Err(fault.to_vec())),
        },
        edit @ (Command::Hide | Command::Unhide) => match hide::read(rest) {
            hide::Asked::Help => Words::Answered(help::command(word)),
            hide::Asked::Paths(words) => Words::Read(Ok(Read::Edit(edit, words))),
            hide::Asked::Misused(fault) => Words::Read(Err(fault.text())),
        },
        Command::Status => with_table(&status::TABLE, Operands::Paths, word, rest, Read::Status),
        Command::Add => with_table(&add::TABLE, Operands::Paths, word, rest, |read| {
            Read::Add(read, rest)
        }),
        Command::Clean => with_table(&clean::TABLE, Operands::Pathspecs, word, rest, |read| {
            Read::Clean(read, rest)
        }),
        Command::Git => match git::read(rest) {
            git::Asked::Help => Words::Answered(help::command(word)),
            git::Asked::Run(words) => Words::Read(Ok(Read::Passed(Passed::Git(words)))),
            git::Asked::GlobalOption => Words::Answered(outcome::usage_error(
                git::GLOBAL_OPTION,
                help::command_usage_line(word),
            )),
            git::Asked::Empty => Words::Read(Err(git::EMPTY.to_vec())),
        },
        Command::Stash => Words::Read(Ok(Read::Passed(Passed::Stash(words)))),
        Command::Push | Command::Pull | Command::Fetch | Command::Remote => {
            Words::Read(Ok(Read::Passed(Passed::Transfer(words))))
        }
    }
}

/// Step (3) where the locate failed: what is answered there without a repository, its
/// runs and the alias lookups made as `Composition/Runner` says: what step (2) answers at
/// once of a command an alias reaches, and a help request after a word passed through,
/// after `stash`, or after a transfer command. `None` for every other command, a held
/// misuse included, which ends with Git's message; a lookup Git failed ends the command as
/// Git ended it. Where something other than a directory stands at the private Git
/// directory, nothing is looked up or run through it, and a help request is refused there.
fn answered_unlocated(
    read: Option<Result<Read, Vec<u8>>>,
    words: &[OsString],
    not_located: &NotLocated,
) -> Option<Outcome> {
    let NotLocated::Refused {
        unlocated: Some(unlocated),
        ..
    } = not_located
    else {
        return None;
    };
    let against = unlocated.against();
    let obstruction = unlocated.obstruction();
    let mut expanded = None;
    let read = match read {
        Some(read) => read,
        None => match through_aliases(words, against, obstruction.is_some(), &mut expanded) {
            Reached::Read(_, read) => read,
            Reached::Answered(answered) | Reached::Failed(answered) => return Some(answered),
        },
    };
    let passed = help_request(&read)?;
    Some(match obstruction {
        Some(obstruction) => outcome::obstructed(obstruction),
        None => passed.run(against, || unlocated.facts()),
    })
}

/// Step (4), located or not: a first word F6 does not name, resolved through the aliases
/// with the lookups made `against` where its run would be made (`alias`), or passed through
/// unresolved where something other than a directory stands at the private Git directory
/// (`obstructed`). A chain's prefix is carried before the command it reaches is read from
/// the expansion, which `expanded` keeps, as step (2) reads that command typed; it keeps
/// too the words a chain that passes through reaches, for the hint after the run.
fn through_aliases<'w>(
    words: &'w [OsString],
    against: Against,
    obstructed: bool,
    expanded: &'w mut Option<Vec<OsString>>,
) -> Reached<'w> {
    let resolved = if obstructed {
        Ok(Resolved::PassThrough)
    } else {
        alias::resolve(words, against)
    };
    match resolved {
        Err(ended) => Reached::Failed(ended),
        Ok(Resolved::PassThrough) => {
            Reached::Read(None, Ok(Read::Passed(Passed::Word(words, words))))
        }
        Ok(Resolved::PassThroughReaching(reached)) => {
            let read = expanded.insert(reached);
            Reached::Read(None, Ok(Read::Passed(Passed::Word(words, read))))
        }
        Ok(Resolved::Dispatch {
            command,
            words: dispatched,
            prefix,
        }) => {
            runner::carry_prefix(prefix);
            match read_words(command, expanded.insert(dispatched)) {
                Words::Answered(answered) => Reached::Answered(answered),
                Words::Read(read) => Reached::Read(Some(command), read),
            }
        }
        Ok(Resolved::Ambiguous(reached)) => {
            Reached::Read(None, Err(alias::ambiguous(&reached, words[0].as_bytes())))
        }
    }
}

/// Step (4) where the locate found the workspace: the lookups are made against its private
/// repository, attached or not (`Holds/G4`); where something other than a directory stands
/// at its path, the word passes through unresolved, and step (5) refuses it.
fn aliased<'w>(
    workspace: &Workspace,
    words: &'w [OsString],
    expanded: &'w mut Option<Vec<OsString>>,
) -> Reached<'w> {
    let private = workspace.private_directory();
    let against = Against::Private {
        git_directory: &private,
        root: workspace.root(),
    };
    through_aliases(words, against, workspace.obstruction().is_some(), expanded)
}

/// After step (4): a command that attaches, reached through an alias, was located as one
/// that does not; it is located again as one that attaches is, which also asks for the
/// superproject (G4). That run carries the chain's prefix, and its failure ends the command
/// as Git ended it, settled where the first locate found the workspace attached (R3).
fn relocated(workspace: Workspace, aliased_attaching: bool) -> Result<Workspace, Outcome> {
    if !aliased_attaching {
        return Ok(workspace);
    }
    locate::locate(true)
        .map_err(|not_located| ended_early(&workspace, not_located_end(not_located)))
}

/// Step (5) in an unattached worktree, for a command that does not attach: refused naming
/// `git dupe init`, but for a help request after a word passed through, after `stash`, or
/// after a transfer command, which Git answers alone, after the command's guard, against
/// this worktree's private Git directory, which no `init` has made, so that it never
/// selects the public repository nor another worktree's private one. Where something other
/// than a directory stands there, Git would follow it, and the request is refused naming it.
fn unattached(workspace: &Workspace, read: &Result<Read, Vec<u8>>) -> Outcome {
    let Some(passed) = help_request(read) else {
        return outcome::refuse_unattached();
    };
    match workspace.obstruction() {
        Some(obstruction) => outcome::obstructed(&obstruction),
        None => run_passed(passed, workspace),
    }
}

/// The usage error of a misuse held from step (2) or (4), or of an operand `handler`
/// refuses: the fault, then the usage line of the command reached, or the general one
/// where no command was reached.
fn misused(fault: &[u8], command: Option<Command>) -> Outcome {
    let usage_line = command.map_or(help::general_usage_line(), |command| {
        help::command_usage_line(command.word())
    });
    outcome::usage_error(fault, usage_line)
}

/// Step (7): the handler.
fn handle(handler: Handler, workspace: &Workspace) -> Outcome {
    match handler {
        Handler::Init(branch) => init::run(workspace, branch),
        Handler::Clone { words, url, branch } => clone::run(workspace, words, url, branch),
        Handler::Hide(paths) => hide::hide(workspace, &paths),
        Handler::Unhide(paths) => hide::unhide(workspace, &paths),
        Handler::Status(read, paths) => status::status(workspace, &read, paths.as_deref()),
        Handler::Add(read, words, operands) => add::add(workspace, &read, words, &operands),
        Handler::Clean(read, words) => clean::clean(workspace, &read, words),
        Handler::Passed(passed) => run_passed(passed, workspace),
        Handler::Detach(_) => unreachable!("detach ends before step (6)"),
    }
}

/// A command Git runs as itself, in step (5) or (7): run against this worktree's private
/// repository, attached or not, its guard comparing from where the workspace stands.
fn run_passed(passed: Passed, workspace: &Workspace) -> Outcome {
    let private = workspace.private_directory();
    let against = Against::Private {
        git_directory: &private,
        root: workspace.root(),
    };
    passed.run(against, || Some(workspace.facts()))
}

/// How a command ends where the workspace was not located: with Git's message and status,
/// or the refusal of an answer that cannot be read or a run that did not start.
fn not_located_end(not_located: NotLocated) -> Outcome {
    match not_located {
        NotLocated::Refused { message, end, .. } => {
            lines::relay(&message);
            Outcome::Git(end)
        }
        NotLocated::Unreadable => outcome::refuse(b"cannot read where Git says this repository is"),
        NotLocated::NotStarted(failure) => outcome::not_started(failure),
    }
}

/// A command that ended before its handler, as Git ended a run it needed: settled all
/// the same in an attached workspace (R3).
fn ended_early(workspace: &Workspace, ended: Outcome) -> Outcome {
    if !workspace.attached() {
        return ended;
    }
    let starting = keeper::start(workspace);
    settle(workspace, &starting, ended)
}

/// A help request after a word passed through, after `stash`, or after a transfer
/// command: what Git answers alone, after the command's guard, where no workspace is
/// attached.
fn help_request<'w>(read: &Result<Read<'w>, Vec<u8>>) -> Option<Passed<'w>> {
    match read {
        Ok(Read::Passed(passed)) if passed.asks_for_help() => Some(*passed),
        _ => None,
    }
}

/// Step (2) for a command with a table: a help request and a word the table does not
/// hold answered at once, and, where the operands are literal paths, one that is no
/// literal path held as a misuse.
fn with_table<'w>(
    table: &'static [Entry],
    operands: Operands,
    first: &[u8],
    rest: &'w [OsString],
    read: impl FnOnce(table::Read<'w>) -> Read<'w>,
) -> Words<'w> {
    match table::read(table, rest) {
        Reading::Help => Words::Answered(help::command(first)),
        Reading::NotHeld(word) => Words::Answered(outcome::usage_error(
            &Fault::NotHeld(word).text(first),
            help::command_usage_line(first),
        )),
        Reading::Read(words) => Words::Read(match operands {
            Operands::Paths => match words.not_literal() {
                Some(fault) => Err(fault.text(first)),
                None => Ok(read(words)),
            },
            Operands::Pathspecs => Ok(read(words)),
        }),
    }
}

/// What the operands of a command with a table are.
#[derive(Clone, Copy)]
enum Operands {
    /// Literal paths, as `status` and `add` read them (G12, G13).
    Paths,
    /// Git's own pathspecs, as `clean` gives them to Git, as typed (`Holds/G16`).
    Pathspecs,
}

/// What step (2) made of a command's words.
enum Words<'w> {
    /// Answered without locating; the command ends with it.
    Answered(Outcome),
    /// What the handler is resolved from, or the fault of a misuse, reported before step
    /// (6).
    Read(Result<Read<'w>, Vec<u8>>),
}

/// What step (4) made of a first word F6 does not name.
enum Reached<'w> {
    /// The command its chain reached, `None` where the words pass through or no release
    /// reads it alike, and what step (2) read of that command's words; or the fault held for
    /// after step (5): that command's misuse, or the usage error G19 names.
    Read(Option<Command>, Result<Read<'w>, Vec<u8>>),
    /// What step (2) answers at once of the command reached: the command ends with it,
    /// unsettled.
    Answered(Outcome),
    /// A lookup Git failed: the command ends as Git ended it, settled by a caller that
    /// located an attached workspace (R3).
    Failed(Outcome),
}

/// What step (2) read from a command's words. A misuse is not one: it is its fault,
/// reported before step (6).
enum Read<'w> {
    /// `init`, with the initial branch when one was given.
    Init(Option<&'w OsStr>),
    /// `clone`, with every word after it, the `URL`, and the branch when one was given.
    Clone {
        words: &'w [OsString],
        url: &'w OsStr,
        branch: Option<&'w OsStr>,
    },
    /// `detach`, with `--force` or without.
    Detach(bool),
    /// `hide` or `unhide`, with its operands as typed.
    Edit(Command, Vec<&'w OsStr>),
    /// `status`, with the words its table holds.
    Status(table::Read<'w>),
    /// `add`, with the words its table holds and the words as typed.
    Add(table::Read<'w>, &'w [OsString]),
    /// `clean`, with the words its table holds and the words as typed.
    Clean(table::Read<'w>, &'w [OsString]),
    /// A command Git runs as itself once its guard has passed.
    Passed(Passed<'w>),
}

/// The handler step (7) runs.
enum Handler<'w> {
    Init(Option<&'w OsStr>),
    Clone {
        words: &'w [OsString],
        url: &'w OsStr,
        branch: Option<&'w OsStr>,
    },
    /// `detach`, with `--force` or without: no command G6 covers.
    Detach(bool),
    /// `hide`, with the root-relative paths its operands name.
    Hide(Vec<Vec<u8>>),
    Unhide(Vec<Vec<u8>>),
    /// `status`, with the root-relative paths its operands name, `None` without one.
    Status(table::Read<'w>, Option<Vec<Vec<u8>>>),
    /// `add`, with the words as typed and its operands, each a form or a root-relative
    /// path.
    Add(table::Read<'w>, &'w [OsString], Vec<add::Operand>),
    /// `clean`, with the words as typed.
    Clean(table::Read<'w>, &'w [OsString]),
    Passed(Passed<'w>),
}

/// The handler for what step (2) read of `command`'s words, its operands resolved
/// against the prefix; or the fault of one that names a path its command refuses there:
/// the root or a path outside it for `hide` and `unhide`, a path outside it for `status`
/// and `add`.
fn handler<'w>(
    read: Read<'w>,
    workspace: &Workspace,
    command: Option<Command>,
) -> Result<Handler<'w>, Vec<u8>> {
    let command = command.map_or(&b""[..], Command::word);
    let paths = |words: &[&OsStr]| hide::resolve(workspace, words).map_err(|fault| fault.text());
    Ok(match read {
        Read::Init(branch) => Handler::Init(branch),
        Read::Clone { words, url, branch } => Handler::Clone { words, url, branch },
        Read::Detach(force) => Handler::Detach(force),
        Read::Edit(Command::Hide, words) => Handler::Hide(paths(&words)?),
        Read::Edit(_, words) => Handler::Unhide(paths(&words)?),
        Read::Status(read) => {
            let paths =
                status::resolve(workspace, &read.operands).map_err(|fault| fault.text(command))?;
            Handler::Status(read, paths)
        }
        Read::Add(read, words) => {
            let operands =
                add::resolve(workspace, &read.operands).map_err(|fault| fault.text(command))?;
            Handler::Add(read, words, operands)
        }
        Read::Clean(read, words) => Handler::Clean(read, words),
        Read::Passed(passed) => Handler::Passed(passed),
    })
}

/// Settles an attached workspace after `handled` and prints the warnings after the
/// handler's lines. The handler's outcome stands, unless a list settle needed did not
/// fit, which refuses the command in its place (G23).
fn settle(workspace: &Workspace, starting: &StartingRegion, handled: Outcome) -> Outcome {
    if !workspace.attached_now() {
        return handled;
    }
    let settled = keeper::settle(workspace, starting);
    for warning in &settled.warnings {
        lines::write(Level::Warning, warning);
    }
    match settled.refused {
        Some(count) => outcome::list_too_long(count),
        None => handled,
    }
}
