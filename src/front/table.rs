//! The words of a command with a table (G26, P1, `Composition/Front` "Option reading"):
//! its options read against the table, its operands, and the lines of their faults. This
//! is the one reading of a dashed word of `status`, `add`, and `clean`; no other code
//! parses one again. Whether an operand is a literal path is the caller's question: those
//! of `status` and `add` are, those of `clean` are Git's own pathspecs.
//!
//! What the code below cannot show:
//!
//! - A table is a constant in its command's file, one entry per option P1 names: its long
//!   name when it has one, its short letter when it has one, and its value form. An entry
//!   without a long name is spelled by its letter alone, never by a long word. A `--no-`
//!   form is an entry of its own and nothing is derived, so an abbreviation, a `--no-`
//!   form the table does not list, and `--end-of-options` are words the table does not
//!   hold (N11, F4).
//! - Words are read left to right. The first of `-h`, `--help`, and an option word the
//!   table does not hold decides, by position, and nothing runs after either. `--` ends
//!   the reading: it is no operand, and every word after it is one. Where it stood among
//!   the words is kept, for a caller that inserts words of its own before it.
//! - A caller asks which entries the option words spelled and never scans the words
//!   itself: `-su` spells `-u`, the value in `-uno` is no option, and neither is the word
//!   after `--chmod`, whatever it is; an option typed last that takes the next word as
//!   its value and finds none is known as such. The words without some entries are
//!   written here too, from the same reading.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use super::lines;
use crate::guards::operand::{self, NotLiteral};

/// How an option takes its value, as P1 gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    /// None: `--verbose`, `-v`.
    None,
    /// Optional and attached only: `--porcelain` or `--porcelain=v2`; `-u` or `-uno`,
    /// where the rest of a short word after the letter is the value.
    Attached,
    /// Attached or the next word: `--chmod=+x` or `--chmod +x`. The next word is the
    /// value whatever it is, `--`, `-h`, and `--help` included; when no word follows,
    /// the option stands as written.
    Next,
}

/// One option of a table.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    short: Option<u8>,
    long: Option<&'static [u8]>,
    value: Value,
}

impl Entry {
    /// An option with a short letter and a long name: `-v` and `--verbose`.
    pub const fn both(short: u8, long: &'static str, value: Value) -> Entry {
        Entry {
            short: Some(short),
            long: Some(long.as_bytes()),
            value,
        }
    }

    /// An option with a long name alone: `--show-stash`.
    pub const fn long(long: &'static str, value: Value) -> Entry {
        Entry {
            short: None,
            long: Some(long.as_bytes()),
            value,
        }
    }

    /// An option with a short letter alone: `clean`'s `-d`. No long word spells it.
    pub const fn short(short: u8, value: Value) -> Entry {
        Entry {
            short: Some(short),
            long: None,
            value,
        }
    }
}

/// What a command's words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Reading<'w> {
    /// `-h` or `--help` came first.
    Help,
    /// An option word the table does not hold came first: the word as typed.
    NotHeld(&'w OsStr),
    Read(Read<'w>),
}

/// Words the table holds.
#[derive(Debug, PartialEq, Eq)]
pub struct Read<'w> {
    /// The option words in the order typed.
    options: Vec<OptionWord<'w>>,
    /// The words before `--` that are no option, then every word after it, in order.
    pub operands: Vec<&'w OsStr>,
    /// Where the `--` that ended the reading stood among the words read, when one did: a
    /// `--` taken as an option's value ends nothing.
    end_of_options: Option<usize>,
    /// The entry of the last word when that word takes the next word as its value and no
    /// word follows: `-e` or `--exclude` typed last.
    lacking_a_value: Option<&'static Entry>,
}

/// One option word as typed, and the entries it spelled, in order: one for a long word,
/// one per letter read for a bundle; and the next word when it was taken as the value.
#[derive(Debug, PartialEq, Eq)]
struct OptionWord<'w> {
    word: &'w OsStr,
    spelled: Vec<&'static Entry>,
    value: Option<&'w OsStr>,
}

impl<'w> Read<'w> {
    /// The option words in the order typed, each as typed, a value taken as the next
    /// word standing after its option.
    pub fn options(&self) -> impl Iterator<Item = &'w OsStr> + '_ {
        self.options
            .iter()
            .flat_map(|option| [Some(option.word), option.value].into_iter().flatten())
    }

    /// Whether the option words spell `entry`: alone, in a bundle, or with a value.
    pub fn spells(&self, entry: &Entry) -> bool {
        self.options
            .iter()
            .any(|option| option.spelled.contains(&entry))
    }

    /// The last of `entries` the option words spelled, read left to right and letter by
    /// letter: what a last-wins flag ends as.
    pub fn last_of(&self, entries: &[&Entry]) -> Option<&'static Entry> {
        self.options
            .iter()
            .flat_map(|option| &option.spelled)
            .rev()
            .find(|spelled| entries.contains(spelled))
            .copied()
    }

    /// The option words as `options` gives them, without every spelling of `entries`: a
    /// long word spelling one dropped with its value, a letter spelling one taken out of
    /// its bundle, with the value it holds, and a bundle left without letters dropped.
    pub fn options_without(&self, entries: &[&Entry]) -> Vec<OsString> {
        let mut kept = Vec::new();
        for option in &self.options {
            let word = option.word.as_bytes();
            let spells_one = |entry: &&'static Entry| entries.contains(entry);
            if word.starts_with(b"--") {
                if !option.spelled.iter().any(spells_one) {
                    kept.extend(
                        [Some(option.word), option.value]
                            .into_iter()
                            .flatten()
                            .map(OsStr::to_owned),
                    );
                }
                continue;
            }
            // A short word is `-`, one letter per entry spelled, then the value the last
            // letter holds, if any.
            let letters = &word[1..=option.spelled.len()];
            let attached = &word[1 + option.spelled.len()..];
            let mut rewritten = b"-".to_vec();
            let mut last_kept = false;
            for (letter, entry) in letters.iter().zip(&option.spelled) {
                last_kept = !spells_one(entry);
                if last_kept {
                    rewritten.push(*letter);
                }
            }
            if rewritten.len() == 1 {
                continue;
            }
            if last_kept {
                rewritten.extend_from_slice(attached);
            }
            kept.push(OsString::from_vec(rewritten));
            if last_kept && let Some(value) = option.value {
                kept.push(value.to_owned());
            }
        }
        kept
    }

    /// Where the `--` that ended the reading stood among the words read, when one did.
    pub fn end_of_options(&self) -> Option<usize> {
        self.end_of_options
    }

    /// The entry of the last word, when that word takes the next word as its value and
    /// none follows: the option then stands as written, without one.
    pub fn lacking_a_value(&self) -> Option<&'static Entry> {
        self.lacking_a_value
    }

    /// The first operand that is a usage error whatever the user's directory: a pattern,
    /// or magic other than a leading `:/` (G12, G13).
    pub fn not_literal(&self) -> Option<Fault<'w>> {
        self.operands
            .iter()
            .find_map(|&word| match operand::rooted_or_literal(word.as_bytes()) {
                Ok(()) => None,
                Err(NotLiteral::Pattern) => Some(Fault::Pattern(word)),
                Err(NotLiteral::Magic) => Some(Fault::Magic(word)),
            })
    }
}

/// Reads `words`, the words after the command, against `table`.
pub fn read<'w>(table: &'static [Entry], words: &'w [OsString]) -> Reading<'w> {
    let mut read = Read {
        options: Vec::new(),
        operands: Vec::new(),
        end_of_options: None,
        lacking_a_value: None,
    };
    let mut words = words.iter().enumerate();
    while let Some((at, word)) = words.next() {
        let bytes = word.as_bytes();
        if bytes == b"--" {
            read.end_of_options = Some(at);
            read.operands
                .extend(words.map(|(_, word)| word.as_os_str()));
            break;
        }
        if !bytes.starts_with(b"-") {
            read.operands.push(word);
            continue;
        }
        if bytes == b"-h" || bytes == b"--help" {
            return Reading::Help;
        }
        let spelled = match bytes.strip_prefix(b"--") {
            Some(long) => long_word(table, long),
            None => short_word(table, &bytes[1..]),
        };
        let Some((spelled, takes_next)) = spelled else {
            return Reading::NotHeld(word);
        };
        let value = if takes_next {
            let value = words.next().map(|(_, word)| word.as_os_str());
            if value.is_none() {
                read.lacking_a_value = spelled.last().copied();
            }
            value
        } else {
            None
        };
        read.options.push(OptionWord {
            word,
            spelled,
            value,
        });
    }
    Reading::Read(read)
}

/// The entry a long word spells, its name exactly an entry's: alone, or followed by `=`
/// and a value where the entry takes one; and whether it takes the next word as its value.
fn long_word(table: &'static [Entry], word: &[u8]) -> Option<(Vec<&'static Entry>, bool)> {
    let (name, valued) = match word.iter().position(|&byte| byte == b'=') {
        Some(at) => (&word[..at], true),
        None => (word, false),
    };
    let entry = table.iter().find(|entry| entry.long == Some(name))?;
    match (entry.value, valued) {
        (Value::None, true) => None,
        (Value::Next, false) => Some((vec![entry], true)),
        _ => Some((vec![entry], false)),
    }
}

/// The entries a short word spells, every letter an entry's, read in order: a letter
/// without a value continues the bundle, and a letter with one takes the rest of the word,
/// or, where its value may be the next word and nothing is left, that word. `-` alone
/// spells nothing and is not held.
fn short_word(table: &'static [Entry], letters: &[u8]) -> Option<(Vec<&'static Entry>, bool)> {
    if letters.is_empty() {
        return None;
    }
    let mut spelled = Vec::new();
    for (at, &letter) in letters.iter().enumerate() {
        let entry = table.iter().find(|entry| entry.short == Some(letter))?;
        spelled.push(entry);
        match entry.value {
            Value::None => {}
            Value::Attached => break,
            Value::Next => return Some((spelled, at + 1 == letters.len())),
        }
    }
    Some((spelled, false))
}

/// Why a word of a command with a table is a usage error. Each line names the word as
/// typed, or, when it holds a newline, which no one line can hold, in Git's quoting.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault<'w> {
    /// An option word the table does not hold (G26).
    NotHeld(&'w OsStr),
    /// An operand holding `*`, `?`, or `[`.
    Pattern(&'w OsStr),
    /// An operand beginning with `:` other than `:/`.
    Magic(&'w OsStr),
    /// An operand that resolves outside the working tree.
    Outside(&'w OsStr),
}

impl Fault<'_> {
    /// The text of the `error:` line for a word of `command`. The first three name
    /// `git dupe git` with the command, the route to Git's own reading of the word.
    pub fn text(&self, command: &[u8]) -> Vec<u8> {
        let unguarded = [b"git dupe git ", command].concat();
        let (word, fault): (_, Vec<u8>) = match self {
            Fault::NotHeld(word) => (
                word,
                [
                    b"is not an option git dupe ",
                    command,
                    b" reads; ",
                    &unguarded,
                    b" gives it to Git unguarded",
                ]
                .concat(),
            ),
            Fault::Pattern(word) => (
                word,
                [
                    b"holds '*', '?', or '[', and git dupe ",
                    command,
                    b" takes literal paths; ",
                    &unguarded,
                    b" reads patterns",
                ]
                .concat(),
            ),
            Fault::Magic(word) => (
                word,
                [
                    b"begins with ':', and git dupe ",
                    command,
                    b" reads only ':/' there, as the root; ",
                    &unguarded,
                    b" reads pathspec magic",
                ]
                .concat(),
            ),
            Fault::Outside(word) => (word, b"lies outside the working tree".to_vec()),
        };
        [&lines::typed(word.as_bytes())[..], b" ", &fault].concat()
    }
}

#[cfg(test)]
mod tests;
