//! What `README.md` tells a developer to type is what the scenarios run. The README's code
//! stands in fenced blocks of two kinds: a `sh` block, every line of which is a command
//! line as a reader copies it, with no `$ ` prompt, and a `text` block, which holds none
//! and no `git dupe`. A command line may end with a comment for the reader, after
//! `COMMENT`, which a shell does not run. Each command line is taken from the README as the
//! repository holds it, without that comment, and compared, byte for byte and in order:
//!
//! - The `git dupe` lines are the page's command lines from its first, in the page's
//!   order and none left out, as far as the README goes: the quick start's, then, when
//!   the README goes on, the examples' from their first. Typed in order, they are what
//!   `manual_page_commands` runs, in the state the texts describe. A command line of a
//!   page text that changes fails here until the README follows it.
//! - The other lines are, in order, `UNPACKING`, the install note's command lines as
//!   `release_archive` follows them, all of them and as the note holds them, and
//!   `FROM_SOURCE`. A changed line of the note fails here until the README follows it.
//!
//! Outside the fenced blocks no line begins with `$ `, and no indented line, which
//! Markdown can read as code, holds `git dupe`, so that no command the README shows
//! escapes the comparison.

use std::fs;
use std::path::Path;

use crate::harness::{command_lines, install_note, note_commands, page_texts_present};

/// The command lines the README shows before the install note's: unpacking a release
/// archive. No scenario runs them; a line is added here once it has been run as written.
const UNPACKING: [&str; 2] = ["tar -xf git-dupe-<version>.tar", "cd git-dupe-<version>"];

/// The command line the README shows after the install note's: installing from a clone.
/// No scenario runs it; a line is added here once it has been run as written.
const FROM_SOURCE: [&str; 1] = ["cargo install --path ."];

/// What opens a comment after a command line: the command is what stands before it.
const COMMENT: &str = "  # ";

/// The marks that open a fenced code block, each closed by a line of the same mark.
const FENCES: [&str; 2] = ["```", "~~~"];

/// The README as the repository holds it.
fn readme() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    fs::read_to_string(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()))
}

/// The kinds of fenced block the README holds.
#[derive(Clone, Copy)]
enum Block {
    /// Command lines alone.
    Shell,
    /// No command line: a diagram, a file's text.
    Text,
}

/// The command lines of `readme`, in order, each without its comment.
fn readme_command_lines(readme: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut open: Option<(&str, Block)> = None;
    for (index, line) in readme.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim_start();
        match open {
            None => {
                if let Some(fence) = FENCES.into_iter().find(|fence| trimmed.starts_with(fence)) {
                    let block = match trimmed[fence.len()..].trim() {
                        "sh" => Block::Shell,
                        "text" => Block::Text,
                        other => panic!(
                            "README.md:{number}: a fenced block of kind {other:?}; it is \
                             `sh`, command lines alone, or `text`, holding none"
                        ),
                    };
                    open = Some((fence, block));
                }
                assert!(
                    !trimmed.starts_with("$ "),
                    "README.md:{number}: a command line outside a fenced block: {line}"
                );
                assert!(
                    !((line.starts_with("    ") || line.starts_with('\t'))
                        && line.contains("git dupe")),
                    "README.md:{number}: an indented line, which can be code, that no fence \
                     marks: {line}"
                );
            }
            Some((fence, block)) => {
                let closes = trimmed.starts_with(fence)
                    && trimmed
                        .trim_end()
                        .bytes()
                        .all(|byte| byte == fence.as_bytes()[0]);
                if closes {
                    open = None;
                    continue;
                }
                match block {
                    Block::Shell => {
                        assert!(
                            !trimmed.is_empty() && !trimmed.starts_with('$'),
                            "README.md:{number}: a line of a `sh` block that is not a command \
                             line as a reader copies it: {line}"
                        );
                        commands.push(
                            trimmed
                                .split_once(COMMENT)
                                .map_or(trimmed, |(command, _)| command.trim_end()),
                        );
                    }
                    Block::Text => {
                        assert!(
                            !trimmed.starts_with("$ "),
                            "README.md:{number}: a command line in a `text` block: {line}"
                        );
                        assert!(
                            !line.contains("git dupe"),
                            "README.md:{number}: `git dupe` in a `text` block: {line}"
                        );
                    }
                }
            }
        }
    }
    assert!(open.is_none(), "README.md: a fenced block left open");
    commands
}

/// Whether `line` runs git-dupe.
fn is_git_dupe(line: &str) -> bool {
    line.starts_with("git dupe ")
}

#[test]
fn the_readme_shows_the_page_s_command_lines_in_the_page_s_order() {
    let readme = readme();
    let shown: Vec<&str> = readme_command_lines(&readme)
        .into_iter()
        .filter(|line| is_git_dupe(line))
        .collect();
    assert!(
        !shown.is_empty(),
        "README.md shows no `git dupe` command line"
    );

    // The page's command lines, as the scenario runs them: each text's in turn.
    let texts = page_texts_present();
    let page: Vec<(&Path, &str)> = texts
        .iter()
        .flat_map(|(path, _, text)| {
            command_lines(text)
                .into_iter()
                .map(|line| (path.as_path(), line))
        })
        .collect();
    for (index, line) in shown.iter().enumerate() {
        let Some(&(path, wanted)) = page.get(index) else {
            panic!("README.md shows `{line}` after the page's last command line");
        };
        assert_eq!(
            *line,
            wanted,
            "README.md shows `{line}` where the page runs `{wanted}`, in {}: the README's \
             `git dupe` lines are the page's from its first, in its order, none left out",
            path.display()
        );
    }
}

#[test]
fn the_readme_s_install_lines_are_the_install_note_s_in_their_order() {
    let readme = readme();
    let shown: Vec<&str> = readme_command_lines(&readme)
        .into_iter()
        .filter(|line| !is_git_dupe(line))
        .collect();
    let note = fs::read_to_string(install_note()).unwrap();
    let note = note_commands(&note);
    assert!(!note.is_empty(), "the install note has no command line");
    let wanted: Vec<&str> = UNPACKING
        .into_iter()
        .chain(note.iter().map(String::as_str))
        .chain(FROM_SOURCE)
        .collect();
    assert_eq!(
        shown, wanted,
        "README.md's command lines other than `git dupe` are not, in order, the archive \
         unpacked, the install note's lines as the note holds them, and the install from a \
         clone"
    );
}
