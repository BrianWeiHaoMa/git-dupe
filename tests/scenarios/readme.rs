//! What `README.md` tells a developer to type is what the scenarios run. Its command lines
//! are read as `shown_command_lines` reads a document (`harness/documents.rs`), and its
//! `git dupe` lines are the page's command lines from its first, in the page's order and
//! none left out, as far as the README goes: the quick start's, then, when the README goes
//! on, the examples' from their first. Typed in order, they are what
//! `manual_page_commands` runs, in the state the texts describe. A change to the page's
//! command lines as far as the README shows them fails here until the README follows it;
//! a line after the last one it shows is not compared. Its other command lines, the
//! install lines, are pinned by `install_lines`.

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
