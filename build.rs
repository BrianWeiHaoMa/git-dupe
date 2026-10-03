//! The build script: renders the manual page `git-dupe.1` from the help texts and the
//! page's own texts into the build output directory. It lists the texts' two directories
//! instead of naming their files, so that a text added later is in the page at the next
//! build, and it starts no process. It refuses a text holding a line that keeps its
//! layout and is too wide for the page to fit 80 columns, naming the file and the line,
//! and a page text whose name gives no heading (`build/manpage.rs`), naming the file.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "build/manpage.rs"]
mod manpage;

const TEXTS: &str = "src/front/help";

const PAGE_TEXTS: &str = "src/front/page";

fn main() {
    // A directory here is watched with everything in it: a text added, changed, or
    // removed reruns this script.
    for watched in [TEXTS, PAGE_TEXTS, "build", "build.rs"] {
        println!("cargo::rerun-if-changed={watched}");
    }

    let mut refused = Vec::new();
    let mut general = None;
    let mut commands = Vec::new();
    for path in text_files(Path::new(TEXTS)) {
        let text = read(&path, manpage::Kind::Help, &mut refused);
        match path.file_stem().and_then(|stem| stem.to_str()) {
            Some("general") => general = Some(text),
            Some(word) => commands.push((word.to_owned(), text)),
            None => panic!("{}: a text is named <word>.txt", path.display()),
        }
    }
    let mut page_texts = Vec::new();
    for path in text_files(Path::new(PAGE_TEXTS)) {
        let text = read(&path, manpage::Kind::Page, &mut refused);
        match path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(manpage::page_heading)
        {
            Some(heading) => page_texts.push((heading, text)),
            None => refused.push(format!(
                "{}: a page text is named <nn>-<heading>.txt, as 10-quick-start.txt",
                path.display()
            )),
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    let general = general.unwrap_or_else(|| panic!("{TEXTS}/general.txt is missing"));

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR for a build script"));
    let page = out.join("git-dupe.1");
    fs::write(&page, manpage::render(&general, &page_texts, &commands))
        .unwrap_or_else(|cause| panic!("{}: {cause}", page.display()));
}

/// The text at `path`, its lines that keep their layout and are too wide each added to
/// `refused`.
fn read(path: &Path, kind: manpage::Kind, refused: &mut Vec<String>) -> String {
    let text =
        fs::read_to_string(path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
    for (line, width) in manpage::too_wide(&text, kind) {
        refused.push(format!(
            "{}:{line}: a line that keeps its layout is {width} columns, over {}",
            path.display(),
            manpage::KEPT_WIDTH
        ));
    }
    text
}

/// Every `.txt` file of the directory, in the byte order of their names, so that the
/// page does not depend on the order the filesystem lists them in.
fn text_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()))
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect();
    files.sort();
    files
}
