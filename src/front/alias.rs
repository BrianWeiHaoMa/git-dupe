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
//!   loop — so that Git runs it or reports it. A chain that reaches a command of F6 is
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
use crate::runner::records;
use crate::runner::{Against, End, Run};
use expansion::Link;

mod expansion;

/// What step (4) made of a first word F6 does not name.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The words as received run as Git itself, without a prefix.
    PassThrough,
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
    line.extend_from_slice(typed);
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
            Reading::Command | Reading::Shell => return Ok(Resolved::PassThrough),
            Reading::Differs(values) => {
                seen.push(word.clone());
                return Ok(if resolver.any_reaches(&values, &prefix, &seen)? {
                    Resolved::Ambiguous(word)
                } else {
                    Resolved::PassThrough
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
            // Git puts each expansion in front of what remains, the outermost first.
            let words = [command.word().to_vec()]
                .into_iter()
                .chain(link.rest)
                .chain(left.into_iter().rev().flatten())
                .map(OsString::from_vec)
                .chain(after.iter().cloned())
                .collect();
            return Ok(Resolved::Dispatch {
                command,
                words,
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
mod tests {
    use super::*;

    /// A configuration as Git would answer for it: the records configured, then each
    /// `-c alias.<name>=<value>` of the prefix asked under, as Git lists a `-c` last.
    struct Configured {
        records: Vec<(&'static str, Option<&'static str>)>,
        listed: Vec<&'static str>,
        /// The prefix of each lookup, in order.
        lookups: Vec<Vec<OsString>>,
        lists: usize,
        failing: bool,
        /// An option Git cannot evaluate: a lookup carrying it fails.
        refused: Option<&'static str>,
    }

    fn configured(records: &[(&'static str, &'static str)]) -> Configured {
        Configured {
            records: records
                .iter()
                .map(|&(name, value)| (name, Some(value)))
                .collect(),
            listed: vec!["log", "show", "whatchanged", "rev-parse", "commit"],
            lookups: Vec::new(),
            lists: 0,
            failing: false,
            refused: None,
        }
    }

    impl Questions for Configured {
        type Failure = &'static str;

        fn aliases(&mut self, prefix: &[OsString]) -> Result<Vec<Record>, &'static str> {
            self.lookups.push(prefix.to_vec());
            if self.failing
                || self
                    .refused
                    .is_some_and(|refused| prefix.iter().any(|word| word == refused))
            {
                return Err("lookup");
            }
            let mut found: Vec<Record> = self
                .records
                .iter()
                .map(|&(name, value)| {
                    (
                        name.as_bytes().to_vec(),
                        value.map(|v| v.as_bytes().to_vec()),
                    )
                })
                .collect();
            for pair in prefix.windows(2) {
                let setting = pair[1].as_bytes();
                if pair[0] == "-c"
                    && let Some(setting) = setting.strip_prefix(b"alias.")
                {
                    let at = setting.iter().position(|&b| b == b'=').expect("a value");
                    found.push((setting[..at].to_vec(), Some(setting[at + 1..].to_vec())));
                }
            }
            Ok(found)
        }

        fn commands(&mut self) -> Result<BTreeSet<Vec<u8>>, &'static str> {
            self.lists += 1;
            Ok(self
                .listed
                .iter()
                .map(|word| word.as_bytes().to_vec())
                .collect())
        }
    }

    fn owned(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    fn resolved(config: &mut Configured, typed: &[&str]) -> Resolved {
        resolve_with(&owned(typed), config).expect("no failure")
    }

    fn dispatch(command: Command, words: &[&str], prefix: &[&str]) -> Resolved {
        Resolved::Dispatch {
            command,
            words: owned(words),
            prefix: owned(prefix),
        }
    }

    #[test]
    fn a_word_without_a_record_or_with_a_shell_alias_passes_through() {
        let mut config = configured(&[("sh1", "!echo x"), ("Other", "status")]);
        assert_eq!(resolved(&mut config, &["log", "-1"]), Resolved::PassThrough);
        assert_eq!(resolved(&mut config, &["nosuch"]), Resolved::PassThrough);
        assert_eq!(resolved(&mut config, &["sh1"]), Resolved::PassThrough);
        // A shell alias and a word without a record need no list.
        assert_eq!(config.lists, 0);
        assert_eq!(config.lookups.len(), 3);
    }

    #[test]
    fn a_chain_that_reaches_a_command_of_f6_is_dispatched_with_its_words() {
        let mut config = configured(&[
            ("sp", "status --porc"),
            ("aa", "add -A"),
            ("su", "stash -u"),
            ("s", "stash"),
            ("wipe", "clean -fdx"),
            ("g", "git status --porc"),
            ("st", "stage notes"),
            ("i", "init"),
        ]);
        let cases: [(&[&str], Command, &[&str]); 9] = [
            (&["sp"], Command::Status, &["status", "--porc"]),
            (&["SP", "-v"], Command::Status, &["status", "--porc", "-v"]),
            (&["aa"], Command::Add, &["add", "-A"]),
            (&["su"], Command::Stash, &["stash", "-u"]),
            (&["s", "-u"], Command::Stash, &["stash", "-u"]),
            (&["wipe", "-h"], Command::Clean, &["clean", "-fdx", "-h"]),
            (&["g"], Command::Git, &["git", "status", "--porc"]),
            (&["st"], Command::Add, &["add", "notes"]),
            (&["i"], Command::Init, &["init"]),
        ];
        for (typed, command, words) in cases {
            assert_eq!(
                resolved(&mut config, typed),
                dispatch(command, words, &[]),
                "{typed:?}"
            );
        }
    }

    #[test]
    fn each_expansion_stands_before_what_remains_and_its_options_lead_the_command() {
        let mut config = configured(&[
            ("a", "-c x.y=1 b two"),
            ("b", "-p status one"),
            ("st", "-c status.short=true status"),
        ]);
        assert_eq!(
            resolved(&mut config, &["a", "three"]),
            dispatch(
                Command::Status,
                &["status", "one", "two", "three"],
                &["-c", "x.y=1", "-p"]
            )
        );
        // The second lookup carried the first link's options, and a third the last link's
        // before the dispatch; the list was asked once.
        assert_eq!(
            config.lookups,
            [
                owned(&[]),
                owned(&["-c", "x.y=1"]),
                owned(&["-c", "x.y=1", "-p"])
            ]
        );
        assert_eq!(config.lists, 1);
        assert_eq!(
            resolved(&mut config, &["st"]),
            dispatch(Command::Status, &["status"], &["-c", "status.short=true"])
        );
    }

    #[test]
    fn options_no_lookup_carried_are_evaluated_before_the_dispatch() {
        let mut config = configured(&[
            ("bad", "--config-env=broken status -h"),
            ("ok", "-p status"),
            ("outer", "-c x.y=1 inner"),
            ("inner", "status"),
        ]);
        config.refused = Some("--config-env=broken");
        assert_eq!(resolve_with(&owned(&["bad"]), &mut config), Err("lookup"));
        assert_eq!(
            config.lookups,
            [owned(&[]), owned(&["--config-env=broken"])]
        );
        config.lookups.clear();
        assert_eq!(
            resolved(&mut config, &["ok"]),
            dispatch(Command::Status, &["status"], &["-p"])
        );
        assert_eq!(config.lookups, [owned(&[]), owned(&["-p"])]);
        // Options a later link's lookup carried are not asked about again.
        config.lookups.clear();
        assert_eq!(
            resolved(&mut config, &["outer"]),
            dispatch(Command::Status, &["status"], &["-c", "x.y=1"])
        );
        assert_eq!(config.lookups, [owned(&[]), owned(&["-c", "x.y=1"])]);
    }

    #[test]
    fn a_later_lookup_sees_the_alias_a_leading_option_defines() {
        let mut config = configured(&[("a", "-c alias.b=stash\\ -u b")]);
        assert_eq!(
            resolved(&mut config, &["a"]),
            dispatch(
                Command::Stash,
                &["stash", "-u"],
                &["-c", "alias.b=stash -u"]
            )
        );
    }

    #[test]
    fn a_chain_git_cannot_follow_passes_the_typed_words_through() {
        let mut config = configured(&[
            ("l1", "l2"),
            ("l2", "l1"),
            ("self", "self"),
            ("bad", "status 'x"),
            ("env", "--git-dir=. status"),
            ("q", "--exec-path status"),
            ("v", "--version"),
            ("empty", ""),
            ("blanks", "   "),
            ("lead", " status"),
            ("opts", "-c x.y=1"),
            ("down", "log --oneline"),
        ]);
        config.records.push(("novalue", None));
        for typed in [
            "l1", "self", "bad", "env", "q", "v", "empty", "blanks", "lead", "opts", "down",
            "novalue",
        ] {
            assert_eq!(
                resolved(&mut config, &[typed]),
                Resolved::PassThrough,
                "{typed}"
            );
        }
    }

    #[test]
    fn a_loop_ends_without_another_lookup() {
        let mut config = configured(&[("l1", "l2"), ("l2", "l1")]);
        assert_eq!(resolved(&mut config, &["l1"]), Resolved::PassThrough);
        assert_eq!(config.lookups.len(), 2);
    }

    #[test]
    fn a_listed_word_with_a_record_is_refused_only_where_a_chain_reaches_f6() {
        let mut config = configured(&[
            ("log", "add -A"),
            ("show", "log"),
            ("whatchanged", "stash -u"),
            ("a", "whatchanged"),
            ("b", "commit -m x"),
        ]);
        config.records.push(("commit", Some("show")));
        assert_eq!(
            resolved(&mut config, &["log"]),
            Resolved::Ambiguous(b"log".to_vec())
        );
        assert_eq!(
            resolved(&mut config, &["whatchanged"]),
            Resolved::Ambiguous(b"whatchanged".to_vec())
        );
        // One link down.
        assert_eq!(
            resolved(&mut config, &["a"]),
            Resolved::Ambiguous(b"whatchanged".to_vec())
        );
        // `show` is listed and aliased to `log`, which is listed and aliased to `add -A`.
        assert_eq!(
            resolved(&mut config, &["show"]),
            Resolved::Ambiguous(b"show".to_vec())
        );
        // `commit` reaches `show`, `log`, then `add`: refused too; `b` reaches `commit`.
        assert_eq!(
            resolved(&mut config, &["b"]),
            Resolved::Ambiguous(b"commit".to_vec())
        );
        let mut config = configured(&[("log", "show"), ("show", "!echo")]);
        assert_eq!(resolved(&mut config, &["log", "-1"]), Resolved::PassThrough);
    }

    #[test]
    fn a_form_only_some_releases_read_is_refused_only_where_a_chain_reaches_f6() {
        let mut config = configured(&[
            ("wipe.command", "stash -u"),
            (".w", "stash -u"),
            ("Foo.x", "stash -u"),
            ("both", "log"),
            ("both.command", "stash -u"),
            ("lg.command", "log --oneline"),
            ("Up.command", "add -A"),
        ]);
        for (typed, named) in [
            ("wipe", "wipe"),
            ("w", "w"),
            ("W", "W"),
            ("foo.x", "foo.x"),
            ("both", "both"),
            ("wipe.command", "wipe.command"),
            ("Up", "Up"),
        ] {
            assert_eq!(
                resolved(&mut config, &[typed]),
                Resolved::Ambiguous(named.as_bytes().to_vec()),
                "{typed}"
            );
        }
        // `.command` compares exactly; the dotted form without regard to case.
        assert_eq!(resolved(&mut config, &["up"]), Resolved::PassThrough);
        assert_eq!(resolved(&mut config, &["lg"]), Resolved::PassThrough);
    }

    #[test]
    fn a_chain_from_a_word_read_differently_follows_plain_links_and_its_own_prefix() {
        let mut config = configured(&[
            ("odd.command", "-c alias.inner=stash\\ -u outer"),
            ("outer", "inner"),
        ]);
        assert_eq!(
            resolved(&mut config, &["odd"]),
            Resolved::Ambiguous(b"odd".to_vec())
        );
        assert_eq!(
            config.lookups.last(),
            Some(&owned(&["-c", "alias.inner=stash -u"]))
        );
        assert_eq!(config.lists, 1);
    }

    #[test]
    fn a_failed_question_ends_the_resolution() {
        let mut config = configured(&[]);
        config.failing = true;
        assert_eq!(resolve_with(&owned(&["log"]), &mut config), Err("lookup"));
    }

    #[test]
    fn the_usage_error_names_the_alias_and_the_unguarded_route() {
        let same = ambiguous(b"log", b"log");
        assert!(same.starts_with(b"'log' is an alias"));
        assert!(same.ends_with(b"'git dupe git log' runs it as this Git reads it"));
        let reached = ambiguous(b"whatchanged", b"a");
        assert!(reached.starts_with(b"'a' leads to the alias 'whatchanged'"));
        assert!(reached.ends_with(b"'git dupe git a' runs it as this Git reads it"));
    }
}
