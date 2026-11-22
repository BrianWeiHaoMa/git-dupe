//! The command lines a Markdown document of this repository shows, read as a reader
//! copies them, for the checks that pin them to what the scenarios run. A document's code
//! stands in fenced blocks of three kinds: a `sh` block and a `powershell` block, every
//! line of which is a command line as a reader copies it, with no `$ ` or `PS> ` prompt,
//! and a `text` block, which holds none and no `git dupe`. A command line of a `powershell`
//! block may end with a comment for the reader, after `COMMENT`, which PowerShell does not
//! run. A line of a `sh` block holds no comment at all, a plain `git` line's included: an
//! interactive zsh, macOS's default shell, reads a `#` as a word of the command unless
//! `interactive_comments` is set, so that a pasted line fails or acts on words of the
//! comment (`holds_a_comment`). Outside the fenced
//! blocks no line begins with a prompt, and no indented line, which Markdown can read as
//! code, holds `git dupe`, so that no command a document shows escapes the checks. A
//! document that breaks one of these rules fails the check that reads it, naming the
//! document and the line.

use std::fs;
use std::path::Path;

/// What opens a comment after a command line: the command is what stands before it.
const COMMENT: &str = "  # ";

/// The marks that open a fenced code block, each closed by a line of the same mark.
const FENCES: [&str; 2] = ["```", "~~~"];

/// What a reader of `sh` and of PowerShell sees before a command in a transcript.
const PROMPTS: [&str; 2] = ["$ ", "PS> "];

/// The kinds of fenced block a document holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Block {
    /// Command lines alone, for a POSIX shell.
    Shell,
    /// Command lines alone, for PowerShell.
    PowerShell,
    /// No command line: a diagram, a file's text.
    Text,
}

/// The command lines of `document`, a path from the repository root such as
/// `docs/install.md`, as the repository holds it: every line of its `sh` and `powershell`
/// blocks, in order, each without its indentation, and a `powershell` line without its
/// comment. A trailing space on a
/// line with no comment is kept, so that it fails the comparison it reaches.
pub fn shown_command_lines(document: &str) -> Vec<String> {
    shown_lines(document)
        .into_iter()
        .map(|(_, line)| line)
        .collect()
}

/// The command lines `shown_command_lines` gives that stand in a `powershell` block.
pub fn shown_powershell_lines(document: &str) -> Vec<String> {
    shown_lines(document)
        .into_iter()
        .filter(|(block, _)| *block == Block::PowerShell)
        .map(|(_, line)| line)
        .collect()
}

/// Whether `line`, with its indentation removed, begins with a prompt.
fn is_prompted(line: &str) -> bool {
    PROMPTS.iter().any(|prompt| line.starts_with(prompt))
}

/// Every command line of `document` with the kind of block it stands in.
fn shown_lines(document: &str) -> Vec<(Block, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(document);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
    lines_of(document, &text)
}

/// The command lines of `text`, which `document` names in what a broken rule says.
fn lines_of(document: &str, text: &str) -> Vec<(Block, String)> {
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
                        "powershell" => Block::PowerShell,
                        "text" => Block::Text,
                        other => panic!(
                            "{document}:{number}: a fenced block of kind {other:?}; it is \
                             `sh` or `powershell`, command lines alone, or `text`, holding \
                             none"
                        ),
                    };
                    open = Some((fence, block, number));
                }
                assert!(
                    !is_prompted(trimmed),
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
                    Block::Shell | Block::PowerShell => {
                        // A PowerShell command may begin with a variable, `$env:Path`.
                        assert!(
                            !trimmed.is_empty()
                                && !is_prompted(trimmed)
                                && (block == Block::PowerShell || !trimmed.starts_with('$')),
                            "{document}:{number}: a line of a command block that is not a \
                             command line as a reader copies it: {line}"
                        );
                        let command = match block {
                            Block::PowerShell => trimmed
                                .split_once(COMMENT)
                                .map_or(trimmed, |(command, _)| command.trim_end()),
                            _ => {
                                assert!(
                                    !holds_a_comment(trimmed),
                                    "{document}:{number}: a comment on a line of a `sh` \
                                     block, which an interactive zsh, macOS's default shell, \
                                     passes to the command as words: {line}"
                                );
                                trimmed
                            }
                        };
                        commands.push((block, command.to_owned()));
                    }
                    Block::Text => {
                        assert!(
                            !is_prompted(trimmed),
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

/// Whether `line`, a line of a `sh` block, holds a comment: a `#` outside quotes that begins
/// a word, after the line's start, whitespace, or an operator, a redirection's included, as
/// POSIX `sh` reads one, a command substitution, `$(…)` or `` `…` ``, reading its own words
/// so inside double quotes too, and continuing the word it stands in. A `#` inside quotes,
/// after a backslash, or inside a word is none.
fn holds_a_comment(line: &str) -> bool {
    // What each open context is: a quote, or a substitution, `(` or `` ` ``, whose words are
    // a command's.
    let mut open: Vec<char> = Vec::new();
    let mut escaped = false;
    let mut word_starts = true;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if escaped {
            escaped = false;
            word_starts = false;
            continue;
        }
        let substitution = match character {
            '$' if characters.peek() == Some(&'(') => {
                characters.next();
                Some('(')
            }
            '`' if open.last() != Some(&'`') && open.last() != Some(&'\'') => Some('`'),
            _ => None,
        };
        match open.last() {
            Some('\'') => {
                if character == '\'' {
                    open.pop();
                }
                continue;
            }
            Some('"') => {
                match (character, substitution) {
                    (_, Some(opened)) => {
                        open.push(opened);
                        word_starts = true;
                    }
                    ('\\', _) => escaped = true,
                    ('"', _) => {
                        open.pop();
                    }
                    _ => {}
                }
                continue;
            }
            _ => {}
        }
        if let Some(opened) = substitution {
            open.push(opened);
            word_starts = true;
            continue;
        }
        match character {
            '#' if word_starts => return true,
            '\\' => escaped = true,
            '\'' | '"' => open.push(character),
            ')' if open.last() == Some(&'(') => {
                open.pop();
                word_starts = false;
                continue;
            }
            '`' if open.last() == Some(&'`') => {
                open.pop();
                word_starts = false;
                continue;
            }
            _ => {}
        }
        word_starts = character.is_whitespace() || ";&|()<>".contains(character);
    }
    false
}

/// Whether `line` runs git-dupe.
pub fn is_git_dupe(line: &str) -> bool {
    line.starts_with("git dupe ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Vec<(Block, String)> {
        lines_of("d.md", text)
    }

    #[test]
    fn each_command_block_keeps_its_kind_in_the_document_s_order() {
        let lines = read(
            "```sh\nmake\n```\n\n```powershell\n$env:Path = 'x'  # a comment\n\
             irm u | iex\n```\n",
        );
        assert_eq!(
            lines,
            [
                (Block::Shell, "make".to_owned()),
                (Block::PowerShell, "$env:Path = 'x'".to_owned()),
                (Block::PowerShell, "irm u | iex".to_owned()),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "d.md:3: a comment on a line of a `sh` block")]
    fn a_comment_after_a_sh_command_fails_a_plain_git_line_s_too() {
        read("```sh\ngit dupe init\ngit status  # the project's\n```\n");
    }

    #[test]
    fn a_comment_is_a_hash_beginning_a_word_outside_quotes() {
        for line in [
            "git dupe init # attach",
            "make;# done",
            "# a line",
            "a |#b",
            "x\t#y",
            "echo marker >#note",
            "echo \"$(echo first # leaked)\"",
            "echo $(echo first # leaked)",
            "printf \"%s\" \"`printf value # leaked`\"",
            "echo `echo a #b`",
        ] {
            assert!(holds_a_comment(line), "{line}");
        }
        for line in [
            "git dupe commit -m \"Fix #12\"",
            "git dupe commit -m 'a # b'",
            "echo a\\ #b",
            "git dupe log --format=%h#%s",
            "echo \"a \\\" # b\"",
            "curl -fsSL https://example.com/x.sh#top | sh",
            "echo \"$(echo a#b) # c\"",
            "echo \"(#)\"",
            "printf value $(printf value)#suffix",
            "printf value `printf value`#suffix",
        ] {
            assert!(!holds_a_comment(line), "{line}");
        }
    }

    #[test]
    #[should_panic(expected = "d.md:2: a fenced block of kind \"pwsh\"")]
    fn a_pwsh_fence_fails_naming_the_document_and_the_line() {
        read("\n```pwsh\nx\n```\n");
    }

    #[test]
    #[should_panic(expected = "d.md:1: a fenced block of kind \"\"")]
    fn an_unmarked_fence_fails_naming_the_document_and_the_line() {
        read("```\nx\n```\n");
    }

    #[test]
    #[should_panic(expected = "d.md:2: a line of a command block")]
    fn a_prompt_in_a_powershell_block_fails() {
        read("```powershell\nPS> irm u | iex\n```\n");
    }

    #[test]
    #[should_panic(expected = "d.md:2: a line of a command block")]
    fn a_variable_in_a_sh_block_fails_as_a_prompt_there() {
        read("```sh\n$HOME\n```\n");
    }

    #[test]
    #[should_panic(expected = "d.md:1: a command line outside a fenced block")]
    fn a_powershell_prompt_outside_a_block_fails() {
        read("PS> irm u | iex\n");
    }
}
