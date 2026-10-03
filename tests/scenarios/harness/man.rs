//! How the manual page is asked for, and how a text, a help text or one of the page's own,
//! is found in what `man` shows: under its section's heading, laid out as the renderer
//! lays it out — each line that keeps its layout intact, seven columns in, with the lines
//! beside it; each paragraph of prose with every word whole and in the order written; an
//! empty line where the text has one, and nothing else. The texts are read from the
//! repository when the check runs, so there is no list of them to extend.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::scenario::{Git, Scenario};

/// The renderer of the page, which decides how a text is laid out; the checks ask it
/// rather than repeat its rule.
#[path = "../../../build/manpage.rs"]
pub mod manpage;

use manpage::{Block, Kind};

/// A section's text stands seven columns in; a line that keeps its layout keeps its own
/// indentation after that.
const INDENT: &str = "       ";

/// The terminal the page must fit (P2), and the width of every `man` asked for the
/// installed page.
pub const TERMINAL: usize = 80;

/// The one locale of every `man` here, whatever the machine's.
const LOCALE: (&str, &str) = ("LC_ALL", "C");

/// Every help text present, with its file.
pub fn texts_present() -> Vec<(PathBuf, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/front/help");
    let texts: Vec<(PathBuf, String)> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| {
            let text = fs::read_to_string(&path).unwrap();
            (path, text)
        })
        .collect();
    assert!(texts.len() >= 2, "the general text and the text of `help`");
    texts
}

/// Every one of the page's own texts present, in the order of the page, as its file, its
/// heading, and its text. An entry whose name gives no heading is not a text the build
/// renders, and fails here, named.
pub fn page_texts_present() -> Vec<(PathBuf, String, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/front/page");
    let mut paths: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    let texts: Vec<(PathBuf, String, String)> = paths
        .into_iter()
        .map(|path| {
            let heading = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".txt"))
                .and_then(manpage::page_heading)
                .unwrap_or_else(|| {
                    panic!(
                        "{}: not a page text the build renders, <nn>-<heading>.txt",
                        path.display()
                    )
                });
            let text = fs::read_to_string(&path).unwrap();
            (path, heading, text)
        })
        .collect();
    assert!(!texts.is_empty(), "the quick start and the examples");
    texts
}

/// The command lines of a page text: each of its lines that keeps its layout and begins,
/// after its indentation, with `$ `, without that mark. A line of prose that begins so
/// fails here: the page would fill it into its paragraph, and nothing would run it.
pub fn command_lines(text: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    for block in manpage::blocks(text, Kind::Page) {
        match block {
            Block::Kept(lines) => commands.extend(
                lines
                    .into_iter()
                    .filter_map(|line| line.trim_start().strip_prefix("$ ")),
            ),
            Block::Prose(lines) => {
                for line in lines {
                    assert!(
                        !line.starts_with("$ "),
                        "a command line not indented: {line}"
                    );
                }
            }
            Block::Gap => {}
        }
    }
    commands
}

/// One text, as the repository holds it: `general`, or a command's word.
pub fn text_present(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("src/front/help/{name}.txt"));
    fs::read_to_string(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()))
}

/// The lines `man` shows for a page read as a local file on a terminal `columns` wide,
/// outside any manual path and outside any scenario.
pub fn shown_by_man(page: &Path, columns: usize) -> Vec<String> {
    let man = Command::new("man")
        .arg("-l")
        .arg(page)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env(LOCALE.0, LOCALE.1)
        .env("MANWIDTH", columns.to_string())
        .output()
        .unwrap_or_else(|cause| panic!("this check needs `man`: {cause}"));
    assert!(
        man.status.success(),
        "man -l {}: {}",
        page.display(),
        String::from_utf8_lossy(&man.stderr)
    );
    lines_of(&man.stdout)
}

/// `man git-dupe` and `git dupe --help`, not yet started: each from the scenario's
/// directory, outside any repository, on a terminal `TERMINAL` wide, with `MANPATH`
/// naming `manual_path` and nothing else, so that no page installed elsewhere on this
/// machine can answer. Git turns the second into `git help dupe`, which asks `man` in
/// turn.
pub fn asking_for_the_page<'s>(s: &'s Scenario, manual_path: &Path) -> [Git<'s>; 2] {
    let asked = |run: Git<'s>| {
        run.variable("MANPATH", manual_path)
            .variable(LOCALE.0, LOCALE.1)
            .variable("MANWIDTH", TERMINAL.to_string())
    };
    [
        asked(s.program("man", ["git-dupe"])),
        asked(s.git(["dupe", "--help"])),
    ]
}

/// What a run printed, as lines; it must be text.
pub fn lines_of(printed: &[u8]) -> Vec<String> {
    String::from_utf8(printed.to_vec())
        .unwrap_or_else(|cause| panic!("not text: {cause}"))
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Whether `shown` holds `text`, of the `kind` given, right below the line `heading`, as
/// the renderer lays the text out, with nothing after it but an empty line or the end;
/// the error says where it does not, by the line of `shown`, counted from 1.
pub fn shown_under(shown: &[String], heading: &str, text: &str, kind: Kind) -> Result<(), String> {
    let line = |at: usize| shown.get(at).map(String::as_str);
    let mut at = 1 + shown
        .iter()
        .position(|line| line == heading)
        .ok_or_else(|| format!("no heading {heading:?}"))?;
    for block in manpage::blocks(text, kind) {
        match block {
            Block::Gap => {
                if line(at) != Some("") {
                    return Err(format!(
                        "line {}: {:?} where the text has an empty line",
                        at + 1,
                        line(at)
                    ));
                }
                at += 1;
            }
            Block::Kept(lines) => {
                for kept in lines {
                    let expected = format!("{INDENT}{kept}");
                    if line(at) != Some(expected.as_str()) {
                        return Err(format!("line {}: {:?}, not {expected:?}", at + 1, line(at)));
                    }
                    at += 1;
                }
            }
            Block::Prose(lines) => {
                let mut words = lines.iter().flat_map(|line| line.split_whitespace());
                let mut next = words.next();
                while let Some(expected) = next {
                    let Some(filled) = line(at)
                        .and_then(|line| line.strip_prefix(INDENT))
                        .filter(|rest| !rest.is_empty() && !rest.starts_with(' '))
                    else {
                        return Err(format!(
                            "line {}: {:?} where the prose goes on with {expected:?}",
                            at + 1,
                            line(at)
                        ));
                    };
                    for word in filled.split_whitespace() {
                        match next {
                            Some(expected) if expected == word => next = words.next(),
                            Some(expected) => {
                                return Err(format!(
                                    "line {}: {word:?} where the text has {expected:?}",
                                    at + 1
                                ));
                            }
                            None => {
                                return Err(format!(
                                    "line {}: {word:?} after the paragraph's last word",
                                    at + 1
                                ));
                            }
                        }
                    }
                    at += 1;
                }
            }
        }
    }
    match line(at) {
        None | Some("") => Ok(()),
        Some(more) => Err(format!("line {}: {more:?} after the text", at + 1)),
    }
}

/// Panics naming the first text present, a help text or one of the page's own, that
/// `shown` does not hold under its section's heading.
pub fn every_text_shown(shown: &[String], by: &str) {
    let help_texts = texts_present().into_iter().map(|(path, text)| {
        let heading = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some("general") => manpage::DESCRIPTION.to_owned(),
            Some(word) => manpage::heading(word),
            None => panic!("{}: a text is named <word>.txt", path.display()),
        };
        (path, heading, text, Kind::Help)
    });
    let page_texts = page_texts_present()
        .into_iter()
        .map(|(path, heading, text)| (path, heading, text, Kind::Page));
    for (path, heading, text, kind) in help_texts.chain(page_texts) {
        if let Err(why) = shown_under(shown, &heading, &text, kind) {
            panic!(
                "{} is not in the page as {by} shows it: {why}\n{}",
                path.display(),
                shown.join("\n")
            );
        }
    }
}

/// Whether any line of `printed`, its indentation aside, is a line of the general text
/// other than an empty one. A page that shows the general text holds its usage line and
/// its command list as written.
pub fn holds_a_line_of_the_general_text(printed: &[u8]) -> bool {
    let general = text_present("general");
    let general: Vec<&str> = general
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    String::from_utf8_lossy(printed)
        .lines()
        .any(|line| general.contains(&line.trim()))
}
