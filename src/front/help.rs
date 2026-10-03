//! The help texts, the usage lines, and the `help` command.
//!
//! A text is a file in `help/` beside this one: `general.txt`, and `<command>.txt` named
//! by the command's word as typed. The manual page is rendered from the same files at
//! build time, so what a text says is written nowhere else; what the page says beyond
//! them is in its own texts, the files of `page/`, which nothing here embeds or prints
//! (`build/manpage.rs`). The rules for a text, which the check below holds every file in
//! the directory to:
//!
//! - Its first line is the usage line and begins `usage: git dupe`. That line is what
//!   follows an `error:` line for the command, and the general text's is what bare
//!   `git dupe` prints. No usage line is written anywhere else.
//! - It is printable ASCII and newlines: no tab, at most 80 columns, at most 23 lines,
//!   one final newline. It is printed byte for byte and holds no markup of any kind.
//! - The page lays it out by how it reads on a screen (`build/manpage.rs`): its first
//!   line and every line that begins with a space, a command list or an option table,
//!   keep their layout, and each is at most 73 columns, or the build refuses it; every
//!   other line is prose, which the page fills to the terminal's width; an empty line
//!   separates paragraphs. A sentence's line never begins with a space, which would
//!   keep it out of its paragraph.
//! - It describes only what this build does.
//! - Every file is embedded below, and nothing is embedded that is not a file.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

use super::commands;
use super::outcome::{self, Outcome};
use crate::runner::Run;

const ADD: &str = include_str!("help/add.txt");

const CLEAN: &str = include_str!("help/clean.txt");

const CLONE: &str = include_str!("help/clone.txt");

const DETACH: &str = include_str!("help/detach.txt");

const GENERAL: &str = include_str!("help/general.txt");

const GIT: &str = include_str!("help/git.txt");

const HELP: &str = include_str!("help/help.txt");

const HIDE: &str = include_str!("help/hide.txt");

const INIT: &str = include_str!("help/init.txt");

const STATUS: &str = include_str!("help/status.txt");

const UNHIDE: &str = include_str!("help/unhide.txt");

/// The text of each command that has one, by its word.
const COMMANDS: [(&str, &str); 10] = [
    ("add", ADD),
    ("clean", CLEAN),
    ("clone", CLONE),
    ("detach", DETACH),
    ("git", GIT),
    ("help", HELP),
    ("hide", HIDE),
    ("init", INIT),
    ("status", STATUS),
    ("unhide", UNHIDE),
];

/// What `git dupe -h` and `git dupe help` print.
pub fn general() -> Outcome {
    outcome::print(GENERAL.as_bytes(), Outcome::Answered)
}

/// What bare `git dupe` prints, and what follows the `error:` line of a dashed first
/// word.
pub fn general_usage_line() -> &'static [u8] {
    usage_line(GENERAL)
}

/// A text's first line, its newline included.
fn usage_line(text: &'static str) -> &'static [u8] {
    text.split_inclusive('\n')
        .next()
        .unwrap_or_default()
        .as_bytes()
}

/// What `git dupe COMMAND -h` prints for a command that reads its own words, all of
/// which have a text.
pub fn command(word: &[u8]) -> Outcome {
    outcome::print(
        text_of(word).unwrap_or_default().as_bytes(),
        Outcome::Answered,
    )
}

/// The usage line that follows the `error:` line of a command that reads its own words.
pub fn command_usage_line(word: &[u8]) -> &'static [u8] {
    usage_line(text_of(word).unwrap_or_default())
}

/// The text of the command `word` names, `stage` naming `add` (F6).
fn text_of(word: &[u8]) -> Option<&'static str> {
    let word: &[u8] = match commands::named(word) {
        Some(command) => command.word(),
        None => word,
    };
    COMMANDS
        .iter()
        .find(|(name, _)| name.as_bytes() == word)
        .map(|(_, text)| *text)
}

/// What the words after `help` ask for.
#[derive(Debug, PartialEq, Eq)]
enum Asked<'w> {
    /// No command: the general text.
    General,
    /// `-h` or `--help` before `--`: the help of `help` itself.
    HelpOfHelp,
    About(&'w OsStr),
    /// Not `help [COMMAND]`: an option, which `help` has none of, or a second command.
    Misused,
}

/// `help [COMMAND]`. A help request anywhere before `--` decides first. `--` ends the
/// reading of dashed words, and a word after it is a command; a command cannot begin
/// with `-`, there or before it, so that no word is ever an option of `git help`.
fn read(words: &[OsString]) -> Asked<'_> {
    let end_of_options = words
        .iter()
        .position(|word| word.as_bytes() == b"--")
        .unwrap_or(words.len());
    let (before, after) = words.split_at(end_of_options);
    if before
        .iter()
        .any(|word| matches!(word.as_bytes(), b"-h" | b"--help"))
    {
        return Asked::HelpOfHelp;
    }
    let mut commands = before.iter().chain(after.iter().skip(1));
    match (commands.next(), commands.next()) {
        (None, _) => Asked::General,
        (Some(command), None) if !command.as_bytes().starts_with(b"-") => Asked::About(command),
        _ => Asked::Misused,
    }
}

/// The `help` command. It never locates the workspace: its texts are embedded, and
/// `git help` needs no repository.
pub fn run(words: &[OsString]) -> Outcome {
    match read(words) {
        Asked::General => general(),
        Asked::HelpOfHelp => outcome::print(HELP.as_bytes(), Outcome::Answered),
        Asked::About(command) => about(command),
        Asked::Misused => outcome::usage_error(
            b"'git dupe help' takes at most one command, and no option",
            usage_line(HELP),
        ),
    }
}

fn about(command: &OsStr) -> Outcome {
    if let Some(text) = text_of(command.as_bytes()) {
        return outcome::print(text.as_bytes(), Outcome::Answered);
    }
    // Any other word is Git's to explain, with the streams it needs for its viewer and
    // with its own end.
    match Run::public([OsStr::new("help"), command]).start() {
        Ok(run) => Outcome::Git(run.end),
        Err(failure) => outcome::not_started(failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    fn asked<'w>(words: &'w [OsString]) -> Asked<'w> {
        read(words)
    }

    fn words(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn help_reads_one_command_at_most_and_no_option() {
        assert_eq!(asked(&words(&[])), Asked::General);
        assert_eq!(asked(&words(&["--"])), Asked::General);
        assert_eq!(asked(&words(&["log"])), Asked::About(OsStr::new("log")));
        assert_eq!(
            asked(&words(&["--", "log"])),
            Asked::About(OsStr::new("log"))
        );
        assert_eq!(asked(&words(&[""])), Asked::About(OsStr::new("")));
        for misuse in [
            &["log", "show"][..],
            &["--all"],
            &["-a", "log"],
            &["log", "-x"],
            &["-"],
            &["log", "--", "show"],
            &["--", "-h"],
            &["--", "--all"],
            &["--", "--"],
        ] {
            assert_eq!(asked(&words(misuse)), Asked::Misused, "{misuse:?}");
        }
    }

    #[test]
    fn a_help_request_anywhere_before_the_end_of_options_decides() {
        for request in [
            &["-h"][..],
            &["--help"],
            &["log", "--help"],
            &["log", "show", "-h"],
            &["--all", "-h"],
            &["-h", "--", "a", "b"],
        ] {
            assert_eq!(asked(&words(request)), Asked::HelpOfHelp, "{request:?}");
        }
        assert_eq!(
            asked(&words(&["log", "--", "-h"])),
            Asked::Misused,
            "after `--`, `-h` is no request"
        );
    }

    /// Every file in the texts' directory, by its name without `.txt`.
    fn files() -> BTreeMap<String, String> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/front/help");
        fs::read_dir(&directory)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_str().unwrap();
                let name = name
                    .strip_suffix(".txt")
                    .unwrap_or_else(|| panic!("{name} is not a text: a text is <name>.txt"));
                (name.to_owned(), fs::read_to_string(&path).unwrap())
            })
            .collect()
    }

    #[test]
    fn every_text_file_keeps_the_rules_for_a_text() {
        for (name, text) in files() {
            assert!(
                text.starts_with("usage: git dupe"),
                "{name}: the first line is the usage line"
            );
            assert!(
                text.ends_with('\n') && !text.ends_with("\n\n"),
                "{name}: one final newline"
            );
            assert!(text.lines().count() <= 23, "{name}: at most 23 lines");
            for line in text.lines() {
                assert!(line.len() <= 80, "{name}: over 80 columns: {line}");
                assert!(
                    line.bytes().all(|byte| (b' '..=b'~').contains(&byte)),
                    "{name}: not printable ASCII: {line:?}"
                );
            }
        }
    }

    #[test]
    fn the_embedded_texts_are_the_files_byte_for_byte() {
        let embedded: BTreeMap<String, String> = [("general", GENERAL)]
            .into_iter()
            .chain(COMMANDS)
            .map(|(name, text)| (name.to_owned(), text.to_owned()))
            .collect();
        assert_eq!(embedded.len(), 1 + COMMANDS.len(), "a text embedded twice");
        assert_eq!(embedded, files());
    }

    #[test]
    fn a_text_belongs_to_a_command_of_git_dupes_own_words() {
        for (name, _) in COMMANDS {
            let command = commands::named(name.as_bytes());
            assert_eq!(
                command.map(commands::Command::word),
                Some(name.as_bytes()),
                "{name}.txt describes no command F6 names"
            );
        }
    }

    #[test]
    fn every_command_that_reads_its_own_words_has_a_text() {
        // The ten G24 names.
        for word in [
            "help", "init", "clone", "detach", "hide", "unhide", "status", "add", "clean", "git",
        ] {
            assert!(text_of(word.as_bytes()).is_some(), "{word}");
            assert!(command_usage_line(word.as_bytes()).starts_with(b"usage: git dupe "));
        }
        assert_eq!(text_of(b"stage"), Some(ADD));
    }

    #[test]
    fn a_usage_line_is_its_texts_first_line() {
        assert_eq!(
            usage_line("usage: git dupe x\n\nmore\n"),
            b"usage: git dupe x\n"
        );
        assert!(general_usage_line().starts_with(b"usage: git dupe"));
        assert!(general_usage_line().ends_with(b"\n"));
    }
}
