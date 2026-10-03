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
mod tests {
    use super::*;

    /// A table with `status`'s two value forms, as P1 gives them for `status`.
    const TABLE: [Entry; 9] = [
        Entry::both(b'v', "verbose", Value::None),
        Entry::both(b's', "short", Value::None),
        Entry::both(b'b', "branch", Value::None),
        Entry::long("long", Value::None),
        Entry::long("porcelain", Value::Attached),
        Entry::both(b'u', "untracked-files", Value::Attached),
        Entry::long("ignored", Value::Attached),
        Entry::both(b'M', "find-renames", Value::Attached),
        Entry::long("no-renames", Value::None),
    ];

    fn words(words: &[&[u8]]) -> Vec<OsString> {
        words
            .iter()
            .map(|word| OsStr::from_bytes(word).to_owned())
            .collect()
    }

    fn os(word: &[u8]) -> &OsStr {
        OsStr::from_bytes(word)
    }

    /// The long names of the entries the words spelled, in order.
    fn spelled(given: &[&[u8]]) -> Vec<&'static [u8]> {
        match read(&TABLE, &words(given)) {
            Reading::Read(read) => read
                .options
                .iter()
                .flat_map(|option| &option.spelled)
                .map(|entry| entry.long.expect("every entry here has a long name"))
                .collect(),
            other => panic!("{given:?}: {other:?}"),
        }
    }

    #[test]
    fn an_option_is_held_alone_bundled_and_with_its_value_attached() {
        let both: &[&[u8]] = &[b"short", b"branch"];
        assert_eq!(spelled(&[b"-sb"]), both);
        assert_eq!(spelled(&[b"-vv"]), [b"verbose", b"verbose"]);
        assert_eq!(spelled(&[b"--porcelain=v2"]), [b"porcelain"]);
        assert_eq!(spelled(&[b"--porcelain"]), [b"porcelain"]);
        assert_eq!(spelled(&[b"-uno"]), [&b"untracked-files"[..]]);
        let su: &[&[u8]] = &[b"short", b"untracked-files"];
        assert_eq!(spelled(&[b"-su"]), su);
        assert_eq!(spelled(&[b"-M50"]), [b"find-renames"]);
        assert_eq!(spelled(&[b"--find-renames=50"]), [b"find-renames"]);
        // The rest of a word after a letter with a value is that value, whatever it holds.
        assert_eq!(spelled(&[b"-uvh"]), [&b"untracked-files"[..]]);
        // An entry typed again is spelled again, in a later word as in a bundle.
        let thrice: &[&[u8]] = &[b"verbose", b"long", b"verbose"];
        assert_eq!(spelled(&[b"-v", b"--long", b"-v"]), thrice);
        assert_eq!(spelled(&[b"--no-renames"]), [b"no-renames"]);
        // The empty value as written.
        assert_eq!(spelled(&[b"--porcelain="]), [b"porcelain"]);
    }

    #[test]
    fn options_and_operands_keep_their_order_and_their_spelling() {
        let given = words(&[b"-sb", b"notes", b"--porcelain=v2", b"docs/x", b"-uno"]);
        let Reading::Read(typed) = read(&TABLE, &given) else {
            panic!()
        };
        let options: Vec<_> = typed.options().collect();
        assert_eq!(options, [os(b"-sb"), os(b"--porcelain=v2"), os(b"-uno")]);
        assert_eq!(typed.operands, [os(b"notes"), os(b"docs/x")]);
        let untracked = &TABLE[5];
        assert!(typed.spells(untracked));
        assert!(!typed.spells(&TABLE[6]));
        // A value of `-u` taken as the next word is no value: `normal` is a path.
        for given in [
            &[&b"-u"[..], b"normal"][..],
            &[b"--untracked-files", b"normal"],
        ] {
            let given = words(given);
            let Reading::Read(separate) = read(&TABLE, &given) else {
                panic!()
            };
            assert!(separate.options().eq([os(given[0].as_bytes())]));
            assert_eq!(separate.operands, [os(b"normal")]);
            assert!(separate.spells(untracked));
        }
    }

    #[test]
    fn the_end_of_options_makes_every_later_word_an_operand() {
        let given = words(&[b"-s", b"--", b"--porc", b"-h", b"--", b"x"]);
        let Reading::Read(typed) = read(&TABLE, &given) else {
            panic!()
        };
        assert!(typed.options().eq([os(b"-s")]));
        assert_eq!(
            typed.operands,
            [os(b"--porc"), os(b"-h"), os(b"--"), os(b"x")]
        );
        let alone = words(&[b"--"]);
        assert_eq!(
            read(&TABLE, &alone),
            Reading::Read(Read {
                options: Vec::new(),
                operands: Vec::new(),
                end_of_options: Some(0),
                lacking_a_value: None,
            })
        );
    }

    #[test]
    fn a_word_the_table_does_not_hold_is_named() {
        for word in [
            &b"--porc"[..],
            b"--no-verbose",
            b"--end-of-options",
            b"--short=1",
            b"--verbose=",
            b"-sX",
            b"-",
            b"-vh",
            b"-x",
            b"--porcelian",
            b"---",
            b"--Porcelain",
            b"--help=x",
            b"-\xe9",
        ] {
            let given = words(&[b"notes", word, b"--porc"]);
            assert_eq!(
                read(&TABLE, &given),
                Reading::NotHeld(os(word)),
                "{}",
                word.escape_ascii()
            );
        }
    }

    #[test]
    fn the_first_help_request_or_word_not_held_decides() {
        for (given, decided) in [
            (
                &[&b"--porc"[..], b"-h"][..],
                Reading::NotHeld(os(b"--porc")),
            ),
            (&[b"-h", b"--porc"], Reading::Help),
            (&[b"x", b"--help"], Reading::Help),
            (&[b"-s", b"--help", b"-x"], Reading::Help),
            (&[b"-x", b"--help"], Reading::NotHeld(os(b"-x"))),
        ] {
            assert_eq!(read(&TABLE, &words(given)), decided, "{given:?}");
        }
        let given = words(&[b"--", b"-h"]);
        assert!(matches!(read(&TABLE, &given), Reading::Read(_)));
    }

    #[test]
    fn an_operand_that_is_no_literal_path_is_found_first_in_order() {
        let fault = |given: &[&[u8]]| {
            let given = words(given);
            match read(&TABLE, &given) {
                Reading::Read(read) => read.not_literal().map(|fault| fault.text(b"status")),
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(
            fault(&[b"notes", b":/", b":/notes/x", b"-s", b"--", b"-x"]),
            None
        );
        let expected = Fault::Pattern(os(b"a*")).text(b"status");
        assert_eq!(fault(&[b"x", b"a*", b":y"]), Some(expected));
        let expected = Fault::Magic(os(b":(glob)x")).text(b"status");
        assert_eq!(fault(&[b"--", b":(glob)x", b"a*"]), Some(expected));
        assert!(fault(&[b":/a?"]).is_some());
    }

    /// A table with the third value form, long and short, beside `add`'s all-flag
    /// entries.
    const NEXT: [Entry; 8] = [
        Entry::both(b'n', "dry-run", Value::None),
        Entry::both(b'v', "verbose", Value::None),
        Entry::both(b'A', "all", Value::None),
        Entry::long("no-all", Value::None),
        Entry::long("no-ignore-removal", Value::None),
        Entry::long("chmod", Value::Next),
        Entry::both(b'e', "exclude", Value::Next),
        Entry::both(b'u', "untracked-files", Value::Attached),
    ];

    fn read_next(given: &[&[u8]]) -> Read<'static> {
        let given: &'static [OsString] = Box::leak(words(given).into_boxed_slice());
        match read(&NEXT, given) {
            Reading::Read(read) => read,
            other => panic!("{other:?}"),
        }
    }

    fn all_os(words: &[&[u8]]) -> Vec<OsString> {
        words.iter().map(|word| os(word).to_owned()).collect()
    }

    #[test]
    fn a_value_attached_or_the_next_word_is_taken_whatever_it_is() {
        for (given, options, operands) in [
            (
                &[&b"--chmod"[..], b"+x", b"x"][..],
                &[&b"--chmod"[..], b"+x"][..],
                &[&b"x"[..]][..],
            ),
            (&[b"--chmod=+x", b"x"], &[b"--chmod=+x"], &[b"x"]),
            (&[b"--chmod", b"--", b"x"], &[b"--chmod", b"--"], &[b"x"]),
            (&[b"--chmod", b"-h", b"x"], &[b"--chmod", b"-h"], &[b"x"]),
            (&[b"--chmod", b"--help"], &[b"--chmod", b"--help"], &[]),
            (&[b"--chmod", b"-A"], &[b"--chmod", b"-A"], &[]),
            // Typed last, it stands as written.
            (&[b"x", b"--chmod"], &[b"--chmod"], &[b"x"]),
            (&[b"-e", b"*.o", b"x"], &[b"-e", b"*.o"], &[b"x"]),
            (&[b"-e*.o"], &[b"-e*.o"], &[]),
            (&[b"-ne", b"--"], &[b"-ne", b"--"], &[]),
            (&[b"-nex", b"y"], &[b"-nex"], &[b"y"]),
        ] {
            let read = read_next(given);
            assert!(
                read.options().eq(options.iter().map(|word| os(word))),
                "{given:?}"
            );
            assert_eq!(
                read.operands,
                operands.iter().map(|word| os(word)).collect::<Vec<_>>()
            );
        }
        // A word taken as a value spells no entry and requests no help.
        let read = read_next(&[b"--chmod", b"-A"]);
        assert!(!read.spells(&NEXT[2]));
        assert!(read.spells(&NEXT[5]));
    }

    #[test]
    fn the_last_spelling_of_a_flag_decides() {
        let flag = [&NEXT[2], &NEXT[3], &NEXT[4]];
        let last = |given: &[&[u8]]| read_next(given).last_of(&flag).and_then(|entry| entry.long);
        assert_eq!(last(&[b"-A", b"--no-all", b"-A"]), Some(&b"all"[..]));
        assert_eq!(last(&[b"-A", b"--no-all"]), Some(&b"no-all"[..]));
        assert_eq!(last(&[b"--no-all", b"-nA"]), Some(&b"all"[..]));
        assert_eq!(
            last(&[b"-An", b"--no-ignore-removal", b"-v"]),
            Some(&b"no-ignore-removal"[..])
        );
        assert_eq!(last(&[b"-nv", b"--chmod", b"-A"]), None);
    }

    #[test]
    fn the_words_without_some_entries_keep_every_other_word_as_typed() {
        let flag = [&NEXT[2], &NEXT[3], &NEXT[4]];
        let without = |given: &[&[u8]]| read_next(given).options_without(&flag);
        assert_eq!(without(&[b"-A", b"--all", b"x"]), all_os(&[]));
        assert_eq!(without(&[b"-Av"]), all_os(&[b"-v"]));
        assert_eq!(without(&[b"-vAn"]), all_os(&[b"-vn"]));
        assert_eq!(without(&[b"-nA", b"--no-ignore-removal"]), all_os(&[b"-n"]));
        assert_eq!(without(&[b"-AA", b"--no-all", b"-n"]), all_os(&[b"-n"]));
        assert_eq!(
            without(&[b"--chmod", b"+x", b"-A", b"--chmod=-x", b"-ne", b"-A"]),
            all_os(&[b"--chmod", b"+x", b"--chmod=-x", b"-ne", b"-A"])
        );
        // A letter taken out with the value it holds, and a value kept with its letter.
        let untracked = [&NEXT[7]];
        let read = read_next(&[b"-nuno", b"-vu"]);
        assert_eq!(read.options_without(&untracked), all_os(&[b"-n", b"-v"]));
        let read = read_next(&[b"-nuno", b"-ve", b"x"]);
        assert_eq!(
            read.options_without(&[&NEXT[0]]),
            all_os(&[b"-uno", b"-ve", b"x"])
        );
        assert_eq!(
            read.options_without(&[&NEXT[6]]),
            all_os(&[b"-nuno", b"-v"])
        );
    }

    /// A table with an entry that has a letter and no long name, beside one that takes
    /// the next word.
    const LETTERS: [Entry; 3] = [
        Entry::short(b'd', Value::None),
        Entry::both(b'f', "force", Value::None),
        Entry::both(b'e', "exclude", Value::Next),
    ];

    #[test]
    fn a_letter_without_a_long_name_is_spelled_by_no_long_word() {
        let given = words(&[b"-fd", b"-d"]);
        let Reading::Read(typed) = read(&LETTERS, &given) else {
            panic!()
        };
        assert!(typed.spells(&LETTERS[0]));
        for word in [&b"--d"[..], b"--=x", b"--=", b"--D"] {
            let given = words(&[word]);
            assert_eq!(read(&LETTERS, &given), Reading::NotHeld(os(word)));
        }
    }

    #[test]
    fn the_end_of_options_is_kept_where_it_stood_and_a_value_is_none() {
        let position = |given: &[&[u8]]| {
            let given = words(given);
            match read(&LETTERS, &given) {
                Reading::Read(read) => read.end_of_options(),
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(position(&[b"-f", b"x", b"--", b"--", b"y"]), Some(2));
        // A `--` taken as a value ends nothing; the next one does.
        assert_eq!(position(&[b"-e", b"--", b"--", b"y"]), Some(2));
        assert_eq!(position(&[b"--exclude", b"--"]), None);
        assert_eq!(position(&[b"-f", b"x"]), None);
    }

    #[test]
    fn an_option_typed_last_without_the_value_it_takes_is_known() {
        let lacking = |given: &[&[u8]]| {
            let given = words(given);
            match read(&LETTERS, &given) {
                Reading::Read(read) => read.lacking_a_value(),
                other => panic!("{other:?}"),
            }
        };
        let exclude = Some(&LETTERS[2]);
        assert_eq!(lacking(&[b"-f", b"-e"]), exclude);
        assert_eq!(lacking(&[b"-fde"]), exclude);
        assert_eq!(lacking(&[b"x", b"--exclude"]), exclude);
        assert_eq!(lacking(&[b"-e", b"x"]), None);
        assert_eq!(lacking(&[b"-ex"]), None);
        assert_eq!(lacking(&[b"--exclude="]), None);
        assert_eq!(lacking(&[b"-e", b"--"]), None);
        assert_eq!(lacking(&[b"--", b"-e"]), None);
        assert_eq!(lacking(&[b"-e", b"x", b"-f"]), None);
    }

    #[test]
    fn a_fault_names_the_word_on_one_line_and_the_unguarded_route() {
        let word = os(b"caf\xe9 *");
        for fault in [
            Fault::NotHeld(word),
            Fault::Pattern(word),
            Fault::Magic(word),
            Fault::Outside(word),
        ] {
            let text = fault.text(b"status");
            assert!(text.starts_with(b"'caf\xe9 *' "), "{fault:?}");
            assert!(!text.contains(&b'\n'), "{fault:?}");
            let unguarded = text.windows(19).any(|part| part == b"git dupe git status");
            assert_eq!(unguarded, !matches!(fault, Fault::Outside(_)), "{fault:?}");
        }
        // A word holding a newline is still named, on one line, in Git's quoting.
        let text = Fault::NotHeld(os(b"--a\nb\t\"\\\x1b\xe9")).text(b"add");
        assert!(
            text.starts_with(b"\"--a\\nb\\t\\\"\\\\\\033\xe9\" "),
            "{}",
            text.escape_ascii()
        );
        let text = Fault::Pattern(os(b"a\n*")).text(b"add");
        assert!(text.starts_with(b"\"a\\n*\" "), "{}", text.escape_ascii());
        assert!(!text.contains(&b'\n'));
    }
}
