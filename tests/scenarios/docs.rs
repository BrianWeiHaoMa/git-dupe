//! The `git dupe` command lines the pages under `docs/` show are lines the manual page
//! holds. Every `docs/*.md` is read as `shown_command_lines` reads a document
//! (`harness/documents.rs`), so that each keeps the fenced-block rules, and each `git dupe`
//! line among its command lines is held by one of the page's own texts: it is one of the
//! text's command lines, which `manual_page_commands` runs, or it stands whole in the
//! text's prose, read as the page fills it, each line break a space, and is followed there
//! by the end of the paragraph, whitespace, or a mark that ends a phrase, never by more of a
//! longer command. Unlike `README.md`'s lines (`readme`), the docs' are not typed in
//! order, so they may show any of the page's commands, in any order, and none the page
//! does not hold; a command the page does not spell is written in prose, which is not read
//! here. The pages are found by listing `docs/`, so a new page is held to this without a
//! line here.

use std::fs;
use std::path::Path;

use crate::harness::manpage::{self, Block, Kind};
use crate::harness::{command_lines, is_git_dupe, page_texts_present, shown_command_lines};

/// What may follow a command where a text's prose names it: the paragraph's end,
/// whitespace, or one of these marks, so that a command is not read inside a longer one.
const ENDS_A_PHRASE: [char; 7] = ['.', ',', ';', ':', ')', '"', '\''];

/// Whether `line` is held by one of `texts`, the page's own texts: one of a text's command
/// lines, or a whole command in one of its paragraphs of prose.
fn held_by_the_page(line: &str, texts: &[&str]) -> bool {
    texts.iter().any(|text| {
        command_lines(text).contains(&line)
            || manpage::blocks(text, Kind::Page)
                .into_iter()
                .any(|block| match block {
                    Block::Prose(lines) => stands_whole_in(line, &lines.join(" ")),
                    Block::Kept(_) | Block::Gap => false,
                })
    })
}

/// Whether `command` occurs in `paragraph` followed by its end, whitespace, or a mark
/// that ends a phrase, at any of its occurrences.
fn stands_whole_in(command: &str, paragraph: &str) -> bool {
    paragraph.match_indices(command).any(|(at, _)| {
        paragraph[at + command.len()..]
            .chars()
            .next()
            .is_none_or(|next| next.is_whitespace() || ENDS_A_PHRASE.contains(&next))
    })
}

#[test]
fn every_git_dupe_line_the_docs_show_is_held_by_the_page() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs");
    let mut documents: Vec<String> = fs::read_dir(&directory)
        .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()))
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".md"))
        .map(|name| format!("docs/{name}"))
        .collect();
    documents.sort();
    assert!(
        documents
            .iter()
            .any(|document| document == "docs/install.md"),
        "docs/ lists no page: {documents:?}"
    );

    let present = page_texts_present();
    let texts: Vec<&str> = present.iter().map(|(_, _, text)| text.as_str()).collect();
    for document in &documents {
        for line in shown_command_lines(document) {
            if is_git_dupe(&line) {
                assert!(
                    held_by_the_page(&line, &texts),
                    "{document} shows `{line}`, which no text of the manual page holds as a \
                     command line or a whole command in its prose: show a form the page \
                     spells, or write it in prose"
                );
            }
        }
    }
}

#[test]
fn a_line_is_held_only_as_the_page_spells_it_whole() {
    let present = page_texts_present();
    let texts: Vec<&str> = present.iter().map(|(_, _, text)| text.as_str()).collect();
    for (line, held) in [
        // A command line of the quick start.
        ("git dupe add notes/ AGENTS.md", true),
        // Prose of the worktrees' text, followed by a comma.
        ("git dupe merge agent", true),
        // A command line of the examples, and prose split across a line break.
        ("git dupe pull", true),
        // The page spells it without the slash.
        ("git dupe unhide notes/", false),
        // A prefix of a command line, followed by more of it.
        ("git dupe add notes/ AGENTS.m", false),
        ("git dupe backup", false),
    ] {
        assert_eq!(held_by_the_page(line, &texts), held, "{line}");
    }
}

#[test]
fn prose_holds_a_command_across_a_line_break_and_before_the_end_of_a_phrase() {
    let text = "Run git dupe statuses first, then git\n\
                dupe add notes/ (or git dupe hide x/) and git dupe status.\n\
                \n\
                Last, git dupe log -1\tand git dupe log -2\n\
                \n    \
                $ git dupe push -u origin HEAD\n";
    for (line, held) in [
        // Across a line break.
        ("git dupe add notes/", true),
        ("git dupe hide x/", true),
        // After an occurrence inside a longer word.
        ("git dupe status", true),
        // Before a tab, and at the end of a paragraph.
        ("git dupe log -1", true),
        ("git dupe log -2", true),
        // A command line of the text, whole.
        ("git dupe push -u origin HEAD", true),
        // Followed by more of a longer word or path.
        ("git dupe add notes", false),
        ("git dupe hide x", false),
        // Inside a command line, which is not prose.
        ("git dupe push -u origin", false),
        // Across two paragraphs.
        ("git dupe status. Last", false),
    ] {
        assert_eq!(held_by_the_page(line, &[text]), held, "{line}");
    }
}
