//! What `README.md` tells a developer to type is what the scenarios run. Its command lines
//! are read as `shown_command_lines` reads a document (`harness/documents.rs`), and its
//! `git dupe` lines are the page's command lines from its first, in the page's order and
//! none left out, as far as the README goes: the quick start's, then, when the README goes
//! on, the examples' from their first. Typed in order, they are what
//! `manual_page_commands` runs, in the state the texts describe. A change to the page's
//! command lines as far as the README shows them fails here until the README follows it;
//! a line after the last one it shows is not compared. Its other command lines, the
//! install lines, are pinned by `install_lines`.

use std::fs;
use std::path::Path;

use crate::harness::{command_lines, is_git_dupe, page_texts_present, shown_command_lines};

#[test]
fn the_readme_shows_the_page_s_command_lines_in_the_page_s_order() {
    let shown: Vec<String> = shown_command_lines("README.md")
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
            line,
            wanted,
            "README.md shows `{line}` where the page runs `{wanted}`, in {}: the README's \
             `git dupe` lines are the page's from its first, in its order, none left out",
            path.display()
        );
    }
}

/// The quick start says, before its first command, that the private repository is made
/// empty, with nothing a host may add, and, before the second machine's `clone`, which has
/// a block of its own as the page's has, how `-b` names the project's branch: a reader who
/// pastes each block as they meet it has read both first.
#[test]
fn the_quick_start_says_what_the_remote_is_and_how_clone_names_a_branch_before_them() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let text = fs::read_to_string(&path).unwrap();
    let section = text
        .split_once("\n## Quick start\n")
        .and_then(|(_, after)| after.split("\n## ").next())
        .expect("README.md has a Quick start section");
    // Prose, a block, prose, a block, prose: each block's text after its opening fence.
    let parts: Vec<&str> = section.split("```").collect();
    assert_eq!(parts.len(), 5, "two blocks in the quick start: {section}");
    let words = |prose: &str| prose.split_whitespace().collect::<Vec<_>>().join(" ");
    let before = words(parts[0]);
    for phrase in ["created empty", "no README, license, or ignore file"] {
        assert!(before.contains(phrase), "before the first block: {before}");
    }
    assert!(words(parts[2]).contains("`-b <branch>`"), "{}", parts[2]);
    assert_eq!(parts[3], "sh\ngit dupe clone <url>\n");
    assert!(!parts[1].contains("clone"), "{}", parts[1]);
}
