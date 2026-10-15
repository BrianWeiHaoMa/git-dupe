//! Step (4) of the sequence: a first word F6 does not name, resolved through the aliases
//! Git would apply (G19, `Holds/G19`).
//!
//! What the code below cannot show:
//!
//! - The resolution is pure over two questions it puts to Git through `Questions`: every
//!   alias record under the prefix gathered so far, one `git config -z --get-regexp
//!   '^alias\.'` per link of each chain followed, and the commands Git runs before it
//!   consults an alias, one `git --list-cmds=builtins,main,others` at most, asked without a
//!   prefix because Git resolves a whole chain against the list it started with (S11). A
//!   chain whose last expansion begins with options gets one more lookup, carrying them,
//!   before it is dispatched, so that Git evaluates every option of the prefix before the
//!   command is read: an option Git cannot evaluate ends the command as Git ended it,
//!   where Git refuses the alias, even when the command would answer without running Git.
//!   The number of questions follows the configuration alone (G22, R4).
//! - It ends in one of three ways. The words as received pass through, unread: a word
//!   without a record, a `!` alias, and every chain that cannot be followed — an expansion
//!   Git refuses, one that begins with a global option Git does not run a command after, a
//!   loop — so that Git runs it or reports it. A chain followed to a word that leads to
//!   no command of F6 passes them through too, and carries the words Git's expansion
//!   runs, assembled as for a dispatch, for the hint after the run to read (`advice`):
//!   every command such a hint follows is a builtin, which Git runs before an alias of its
//!   name, so the chain's last word is what ran. A chain that reaches a command of F6 is
//!   dispatched as that command. A word the listed releases do not all read as the same
//!   alias never dispatches: when a chain from any of its records reaches a command of F6,
//!   it is the usage error G19 names, and otherwise its words pass through. A question Git
//!   fails ends the command as Git ended it.
//! - The questions are made where the command's run would be made: privately where the
//!   workspace was located, attached or not, so that only global, system, and private
//!   aliases apply (`Holds/G4`), and as `Composition/Runner` says where the locate failed.
//!   The lookup of a later link carries the prefix, so that a `-c` defining the alias it
//!   reaches applies, and never in the words, where the runner would read `-c` as the
//!   run's own command word.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use super::commands::{self, Command};
use super::outcome::{self, Outcome};
use crate::guards::quoted::shell_word;
use crate::runner::records;
use crate::runner::{Against, End, Run};
use expansion::Link;

mod expansion;

/// What step (4) made of a first word F6 does not name.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The words as received run as Git itself, without a prefix.
    PassThrough,
    /// The words as received run as Git itself, without a prefix, and Git's expansion of
    /// the chain they begin with runs these: the chain's last command word, then what each
    /// expansion left after its command word, the innermost first, then the words typed
    /// after the alias.
    PassThroughReaching(Vec<OsString>),
    /// The command of F6 a chain reached: its words, the command's own word first, and the
    /// global options the chain's expansions began with, in order.
    Dispatch {
        command: Command,
        words: Vec<OsString>,
        prefix: Vec<OsString>,
    },
    /// A word the listed releases read differently, one of whose chains reaches a command of
    /// F6: the usage error G19 names, naming it.
    Ambiguous(Vec<u8>),
}

/// Resolves `words`, the typed word first, with its questions asked `against` the
/// repository the command's run would be made against.
pub fn resolve(words: &[OsString], against: Against) -> Result<Resolved, Outcome> {
    resolve_with(words, &mut Asked { against })
}

/// The `error:` line of a word G19 refuses as ambiguous, `reached` from the `typed` word.
pub fn ambiguous(reached: &[u8], typed: &[u8]) -> Vec<u8> {
    let mut line = Vec::new();
    let quoted = |line: &mut Vec<u8>, word: &[u8]| {
        line.push(b'\'');
        line.extend_from_slice(word);
        line.push(b'\'');
    };
    if reached != typed {
        quoted(&mut line, typed);
        line.extend_from_slice(b" leads to the alias ");
        quoted(&mut line, reached);
        line.extend_from_slice(b", which");
    } else {
        quoted(&mut line, reached);
        line.extend_from_slice(b" is an alias that");
    }
    line.extend_from_slice(
        b" supported Git releases read differently, and one reading leads to a command \
          git-dupe adds, changes, or guards; 'git dupe git ",
    );
    line.extend_from_slice(&shell_word(typed));
    line.extend_from_slice(b"' runs it as this Git reads it");
    line
}

/// One alias record: its key after `alias.`, and its value, `None` for a key without one.
type Record = (Vec<u8>, Option<Vec<u8>>);

/// What the resolution asks Git.
trait Questions {
    type Failure;
    /// Every alias record, in configuration order, under `prefix`.
    fn aliases(&mut self, prefix: &[OsString]) -> Result<Vec<Record>, Self::Failure>;
    /// Every command Git runs before it consults an alias of the same name.
    fn commands(&mut self) -> Result<BTreeSet<Vec<u8>>, Self::Failure>;
}

/// The questions asked of Git.
struct Asked<'a> {
    against: Against<'a>,
}

impl Questions for Asked<'_> {
    type Failure = Outcome;

    /// Exit 1 is "no alias"; any other failure ends the command, Git's message already on
    /// standard error.
    fn aliases(&mut self, prefix: &[OsString]) -> Result<Vec<Record>, Outcome> {
        let found = Run::against(self.against, ["config", "-z", "--get-regexp", r"^alias\."])
            .prefixed(prefix)
            .capture_output()
            .start()
            .map_err(outcome::not_started)?;
        match found.end {
            End::Code(0) => Ok(records::config_records(&found.stdout)
                .filter_map(|(key, value)| {
                    let name = key.strip_prefix(b"alias.")?;
                    Some((name.to_vec(), value.map(<[u8]>::to_vec)))
                })
                .collect()),
            End::Code(1) => Ok(Vec::new()),
            end => Err(Outcome::Git(end)),
        }
    }

    fn commands(&mut self) -> Result<BTreeSet<Vec<u8>>, Outcome> {
        let found = Run::against(self.against, ["--list-cmds=builtins,main,others"])
            .capture_output()
            .start()
            .map_err(outcome::not_started)?;
        match found.end {
            End::Code(0) => Ok(records::command_names(&found.stdout)
                .map(<[u8]>::to_vec)
                .collect()),
            end => Err(Outcome::Git(end)),
        }
    }
}

fn resolve_with<Q: Questions>(
    words: &[OsString],
    questions: &mut Q,
) -> Result<Resolved, Q::Failure> {
    let (typed, after) = words.split_first().expect("a first word");
    let mut resolver = Resolver {
        questions,
        listed: None,
    };
    let mut word = typed.as_bytes().to_vec();
    let mut prefix = Vec::new();
    let mut seen = Vec::new();
    // The words each expansion left after its command word, outermost first.
    let mut left: Vec<Vec<Vec<u8>>> = Vec::new();
    loop {
        let value = match resolver.reading(&word, &prefix)? {
            Reading::Command | Reading::Shell => return Ok(passed_through(word, left, after)),
            Reading::Differs(values) => {
                seen.push(word.clone());
                return Ok(if resolver.any_reaches(&values, &prefix, &seen)? {
                    Resolved::Ambiguous(word)
                } else {
                    passed_through(word, left, after)
                });
            }
            Reading::Plain(value) => value,
        };
        seen.push(word);
        let Some(link) = value.as_deref().and_then(Link::of) else {
            return Ok(Resolved::PassThrough);
        };
        let carried = link.options.is_empty();
        prefix.extend(link.options.into_iter().map(OsString::from_vec));
        if let Some(command) = commands::named(&link.command) {
            // Options no lookup has carried yet are evaluated by one that carries them
            // before the command is read, which may answer without running Git: Git
            // refuses an alias whose leading option it cannot evaluate.
            if !carried {
                resolver.questions.aliases(&prefix)?;
            }
            left.push(link.rest);
            return Ok(Resolved::Dispatch {
                command,
                words: expanded(command.word().to_vec(), left, after),
                prefix,
            });
        }
        if seen.contains(&link.command) {
            return Ok(Resolved::PassThrough);
        }
        left.push(link.rest);
        word = link.command;
    }
}

/// The words of a chain that reached `word`, which Git runs as a command: `word`, then
/// `left`, what each expansion left after its command word, the outermost first, then
/// `after`, the words typed after the alias. Git puts each expansion in front of what
/// remains, so the innermost expansion's words come first.
fn expanded(word: Vec<u8>, left: Vec<Vec<Vec<u8>>>, after: &[OsString]) -> Vec<OsString> {
    [word]
        .into_iter()
        .chain(left.into_iter().rev().flatten())
        .map(OsString::from_vec)
        .chain(after.iter().cloned())
        .collect()
}

/// The words as received passing through at `word`: as typed where no link was followed,
/// and otherwise with the words the chain reached.
fn passed_through(word: Vec<u8>, left: Vec<Vec<Vec<u8>>>, after: &[OsString]) -> Resolved {
    if left.is_empty() {
        Resolved::PassThrough
    } else {
        Resolved::PassThroughReaching(expanded(word, left, after))
    }
}

/// How the listed releases read a word, from its records.
enum Reading {
    /// No record: a command Git runs, or a word it reports.
    Command,
    /// Plainly aliased to a `!` alias, which Git runs whatever the word names.
    Shell,
    /// Plainly aliased and listed in no release's commands: the last record's value, which
    /// every release reads alike.
    Plain(Option<Vec<u8>>),
    /// Listed, or not plainly aliased: the value of every record of the word.
    Differs(Vec<Option<Vec<u8>>>),
}

struct Resolver<'q, Q> {
    questions: &'q mut Q,
    /// The commands Git runs before an alias, once asked.
    listed: Option<BTreeSet<Vec<u8>>>,
}

impl<Q: Questions> Resolver<'_, Q> {
    /// The records of `word` under `prefix`: a key that is the word without regard to ASCII
    /// case, the word followed by `.command`, or `.` and the word without regard to case
    /// (S11). The word is plainly aliased when it holds no dot and every record of it has
    /// the first form.
    fn reading(&mut self, word: &[u8], prefix: &[OsString]) -> Result<Reading, Q::Failure> {
        let records = self.questions.aliases(prefix)?;
        let command = [word, b".command"].concat();
        let of_word: Vec<&Record> = records
            .iter()
            .filter(|(name, _)| {
                name.eq_ignore_ascii_case(word)
                    || *name == command
                    || name
                        .strip_prefix(b".")
                        .is_some_and(|name| name.eq_ignore_ascii_case(word))
            })
            .collect();
        let Some((_, last)) = of_word.last() else {
            return Ok(Reading::Command);
        };
        let plain = !word.contains(&b'.')
            && of_word
                .iter()
                .all(|(name, _)| name.eq_ignore_ascii_case(word));
        if plain && last.as_deref().is_some_and(|value| value.starts_with(b"!")) {
            return Ok(Reading::Shell);
        }
        if plain && !self.listed(word)? {
            return Ok(Reading::Plain(last.clone()));
        }
        Ok(Reading::Differs(
            of_word
                .into_iter()
                .map(|(_, value)| value.clone())
                .collect(),
        ))
    }

    fn listed(&mut self, word: &[u8]) -> Result<bool, Q::Failure> {
        if self.listed.is_none() {
            self.listed = Some(self.questions.commands()?);
        }
        Ok(self
            .listed
            .as_ref()
            .is_some_and(|listed| listed.contains(word)))
    }

    /// Whether a chain from any of `values`, followed by the same lookups under `prefix`,
    /// reaches a command of F6, `seen` holding the words this chain expanded.
    fn any_reaches(
        &mut self,
        values: &[Option<Vec<u8>>],
        prefix: &[OsString],
        seen: &[Vec<u8>],
    ) -> Result<bool, Q::Failure> {
        for value in values {
            if self.reaches(value.as_deref(), prefix, seen)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn reaches(
        &mut self,
        value: Option<&[u8]>,
        prefix: &[OsString],
        seen: &[Vec<u8>],
    ) -> Result<bool, Q::Failure> {
        let Some(link) = value.and_then(Link::of) else {
            return Ok(false);
        };
        if commands::named(&link.command).is_some() {
            return Ok(true);
        }
        if seen.contains(&link.command) {
            return Ok(false);
        }
        let prefix: Vec<OsString> = prefix
            .iter()
            .cloned()
            .chain(link.options.into_iter().map(OsString::from_vec))
            .collect();
        let values = match self.reading(&link.command, &prefix)? {
            Reading::Command | Reading::Shell => return Ok(false),
            Reading::Plain(value) => vec![value],
            Reading::Differs(values) => values,
        };
        let seen = [seen, &[link.command]].concat();
        self.any_reaches(&values, &prefix, &seen)
    }
}

#[cfg(test)]
mod tests;
