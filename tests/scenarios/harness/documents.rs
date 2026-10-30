//! The command lines a Markdown document of this repository shows, read as a reader
//! copies them, for the checks that pin them to what the scenarios run. A document's code
//! stands in fenced blocks of two kinds: a `sh` block, every line of which is a command
//! line as a reader copies it, with no `$ ` prompt, and a `text` block, which holds none
//! and no `git dupe`. A command line may end with a comment for the reader, after
//! `COMMENT`, which a shell does not run. Outside the fenced blocks no line begins with
//! `$ `, and no indented line, which Markdown can read as code, holds `git dupe`, so that
//! no command a document shows escapes the checks. A document that breaks one of these
//! rules fails the check that reads it, naming the document and the line.

use std::fs;
use std::path::Path;

/// What opens a comment after a command line: the command is what stands before it.
const COMMENT: &str = "  # ";

/// The marks that open a fenced code block, each closed by a line of the same mark.
const FENCES: [&str; 2] = ["```", "~~~"];

/// The kinds of fenced block a document holds.
#[derive(Clone, Copy)]
enum Block {
    /// Command lines alone.
    Shell,
    /// No command line: a diagram, a file's text.
    Text,
}

/// The command lines of `document`, a path from the repository root such as
/// `docs/install.md`, as the repository holds it: every line of its `sh` blocks, in
/// order, each without its indentation and its comment. A trailing space on a line with
/// no comment is kept, so that it fails the comparison it reaches.
pub fn shown_command_lines(document: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(document);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
    let mut commands = Vec::new();
    let mut open: Option<(&str, Block, usize)> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim_start();
        match open {
            None => {
                if let Some(fence) = FENCES.into_iter().find(|fence| trimmed.starts_with(fence)) {
                    let block = match trimmed[fence.len()..].trim() {
                        "sh" => Block::Shell,
                        "text" => Block::Text,
                        other => panic!(
                            "{document}:{number}: a fenced block of kind {other:?}; it is \
                             `sh`, command lines alone, or `text`, holding none"
                        ),
                    };
                    open = Some((fence, block, number));
                }
                assert!(
                    !trimmed.starts_with("$ "),
                    "{document}:{number}: a command line outside a fenced block: {line}"
                );
                assert!(
                    !((line.starts_with("    ") || line.starts_with('\t'))
                        && line.contains("git dupe")),
                    "{document}:{number}: an indented line, which can be code, that no fence \
                     marks: {line}"
                );
            }
            Some((fence, block, _)) => {
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
                            "{document}:{number}: a line of a `sh` block that is not a command \
                             line as a reader copies it: {line}"
                        );
                        commands.push(
                            trimmed
                                .split_once(COMMENT)
                                .map_or(trimmed, |(command, _)| command.trim_end())
                                .to_owned(),
                        );
                    }
                    Block::Text => {
                        assert!(
                            !trimmed.starts_with("$ "),
                            "{document}:{number}: a command line in a `text` block: {line}"
                        );
                        assert!(
                            !line.contains("git dupe"),
                            "{document}:{number}: `git dupe` in a `text` block: {line}"
                        );
                    }
                }
            }
        }
    }
    if let Some((_, _, opened)) = open {
        panic!("{document}:{opened}: a fenced block left open");
    }
    commands
}

/// Whether `line` runs git-dupe.
pub fn is_git_dupe(line: &str) -> bool {
    line.starts_with("git dupe ")
}
