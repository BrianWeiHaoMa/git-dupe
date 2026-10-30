//! The install lines the documents show, read as `shown_command_lines` reads a document,
//! and the addresses `install.sh` names. `README.md` and `docs/install.md` each show the
//! pasted line for this repository, which `install` runs; the README shows no other line
//! but `git dupe` lines, which `readme` pins; and `docs/install.md`'s others are, in
//! order, the archive unpacked, the install note's lines as the note holds them, all of
//! them, which `release_archive` follows, and the install from a clone, so that a changed
//! line of the note fails here until `docs/install.md` follows it.

use std::fs;
use std::path::Path;

use crate::harness::{install_note, is_git_dupe, note_commands, shown_command_lines};

/// The command lines `docs/install.md` shows between the pasted line and the install
/// note's: unpacking a release archive. No scenario runs them; a line is added here once
/// it has been run as written.
const UNPACKING: [&str; 2] = [
    "tar -xf git-dupe-<version>-<platform>-<architecture>.tar",
    "cd git-dupe-<version>",
];

/// The command line `docs/install.md` shows after the install note's: installing from a
/// clone. No scenario runs it; a line is added here once it has been run as written.
const FROM_SOURCE: [&str; 1] = ["cargo install --path ."];

/// The repository, as `Cargo.toml` names it: `https://github.com/<owner>/<repository>`.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The line a reader pastes: the script on `main` of the repository, fetched by `curl`
/// and run by `sh`.
fn pasted_line() -> String {
    let named = REPOSITORY
        .strip_prefix("https://github.com/")
        .unwrap_or_else(|| panic!("not a repository on GitHub: {REPOSITORY}"));
    format!("curl -fsSL https://raw.githubusercontent.com/{named}/main/install.sh | sh")
}

/// A file of the repository, as it holds it.
fn repository_text(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()))
}

#[test]
fn the_readme_and_the_install_page_show_the_pasted_line() {
    let pasted = pasted_line();
    let readme: Vec<String> = shown_command_lines("README.md")
        .into_iter()
        .filter(|line| !is_git_dupe(line))
        .collect();
    assert_eq!(
        readme,
        [pasted.as_str()],
        "README.md's command lines other than `git dupe` are not the pasted line alone"
    );
    let shown: Vec<String> = shown_command_lines("docs/install.md")
        .into_iter()
        .filter(|line| !is_git_dupe(line))
        .collect();
    let note = note_commands(&fs::read_to_string(install_note()).unwrap());
    assert!(!note.is_empty(), "the install note has no command line");
    let wanted: Vec<String> = [pasted]
        .into_iter()
        .chain(UNPACKING.map(str::to_owned))
        .chain(note)
        .chain(FROM_SOURCE.map(str::to_owned))
        .collect();
    assert_eq!(
        shown, wanted,
        "docs/install.md's command lines other than `git dupe` are not, in order, the pasted \
         line, the archive unpacked, the install note's lines as the note holds them, and \
         the install from a clone"
    );
}

/// Every address the script names is this repository's: its releases pages, its own raw
/// path in the pasted line, and a section of `docs/install.md` that the page holds, its
/// anchor the one GitHub makes of the heading.
#[test]
fn the_script_names_this_repository_and_sections_the_install_page_holds() {
    let script = repository_text("install.sh");
    let raw = pasted_line()
        .split(' ')
        .find(|word| word.starts_with("https://"))
        .unwrap()
        .to_owned();
    // An address runs to the first byte no address holds; `https://` followed by no host,
    // as in a `case` pattern, names none.
    let addresses: Vec<&str> = script
        .match_indices("https://")
        .map(|(at, _)| {
            let rest = &script[at..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || "._~:/?#@%+=&-".contains(c)))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .filter(|address| address.len() > "https://".len())
        .collect();
    assert!(
        addresses.contains(&format!("{REPOSITORY}/releases").as_str()),
        "install.sh does not name {REPOSITORY}/releases: {addresses:?}"
    );
    let page = repository_text("docs/install.md");
    let anchors: Vec<String> = page
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|heading| {
            heading
                .to_lowercase()
                .chars()
                .filter_map(|c| match c {
                    ' ' => Some('-'),
                    c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
                    _ => None,
                })
                .collect()
        })
        .collect();
    for address in addresses {
        if address == raw {
            continue;
        }
        assert!(
            address.starts_with(&format!("{REPOSITORY}/")),
            "install.sh names {address}, outside {REPOSITORY}"
        );
        if let Some(anchor) =
            address.strip_prefix(&format!("{REPOSITORY}/blob/main/docs/install.md#"))
        {
            assert!(
                anchors.iter().any(|held| held == anchor),
                "install.sh names the section {anchor} of docs/install.md, whose sections are \
                 {anchors:?}"
            );
        }
    }
}
