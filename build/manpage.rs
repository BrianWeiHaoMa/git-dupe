//! The manual page `git-dupe.1`, rendered from the help texts `git dupe help` prints and
//! from the page's own texts, which no command prints.
//!
//! One pure function: texts in, page out, the same bytes for the same texts. The page
//! says nothing the texts do not: its frame is a title, a name line, and headings, and
//! every other line holds the words of a text. A text holds no markup, so how the page
//! lays out a line follows from how the line reads on a screen, decided here and nowhere
//! else (P2):
//!
//! - A line that keeps its layout is a help text's first line, its usage line, and every
//!   line that begins with a space: a command list, an option table, an example command.
//!   The page shows it as written, in no-fill mode, with the lines beside it.
//! - Prose is every other line that is not empty. The page fills it to the terminal's
//!   width, without hyphenation, so that every word arrives whole and in order, and
//!   without padding between words, so that a command keeps its single spaces. Prose
//!   next to a line that keeps its layout starts on a line of its own.
//! - An empty line, or a run of them, separates paragraphs and blocks: one empty line on
//!   the page.
//!
//! `man` shows a section's text seven columns in, so a line that keeps its layout fits
//! a terminal of 80 columns at `KEPT_WIDTH` columns or fewer; the build refuses a text
//! holding a wider one. What a text needs to come through `man` intact is escaped here
//! and nowhere else, so that a text stays plain.
//!
//! The page's own texts are the files of `src/front/page/`, each a section of its own
//! between the description and the commands' sections: a quick start, examples, an
//! explanation. A file is named `<nn>-<heading>.txt`: two digits that give its place,
//! the files standing in the byte order of their names, and its heading in lowercase
//! words joined by `-`, so that `10-quick-start.txt` is the section QUICK START
//! (`page_heading`). Adding one is adding the file. It has no usage line: only its lines
//! that begin with a space keep their layout. Every line of one that begins, after its
//! indentation, with `$ ` is a command as a developer types it, which the scenarios of
//! `tests/scenarios/manual_page_commands.rs` run from the text as written; that file says
//! what such a line may hold.

/// The widest line that keeps its layout: `man`'s seven columns of indentation and this
/// fill a terminal of 80 columns.
pub const KEPT_WIDTH: usize = 73;

/// The heading of the general text's section.
pub const DESCRIPTION: &str = "DESCRIPTION";

/// What a text is to the page, which decides whether its first line keeps its layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A help text, whose first line is its usage line.
    Help,
    /// One of the page's own texts, which has no usage line.
    Page,
}

/// A run of a text's lines, as the page lays it out.
#[derive(Debug, PartialEq, Eq)]
pub enum Block<'t> {
    /// Lines that keep their layout, shown one page line each.
    Kept(Vec<&'t str>),
    /// Lines of prose, filled together into one paragraph.
    Prose(Vec<&'t str>),
    /// One empty line or more.
    Gap,
}

/// Renders the page: the general text as the description, then each of the page's own
/// texts under its heading, then each command's text as its own section, in the order
/// given. A page text is given with its heading, a command's text with its word.
pub fn render(
    general: &str,
    page_texts: &[(String, String)],
    commands: &[(String, String)],
) -> String {
    // Hyphenation off and no padding between words, for the whole page.
    let mut page = String::from(
        ".TH GIT\\-DUPE 1\n.nh\n.ad l\n.SH NAME\ngit\\-dupe \\- the Git command \"git dupe\"\n",
    );
    section(&mut page, DESCRIPTION, general, Kind::Help);
    for (heading, text) in page_texts {
        section(&mut page, heading, text, Kind::Page);
    }
    for (word, text) in commands {
        section(&mut page, &heading(word), text, Kind::Help);
    }
    page
}

/// The heading of a command's section, by the command's word.
pub fn heading(word: &str) -> String {
    format!("GIT DUPE {}", word.to_ascii_uppercase())
}

/// The heading of the page's own text whose file is named `<name>.txt`, or nothing when
/// the name is not two digits, `-`, and lowercase words joined by `-`.
pub fn page_heading(name: &str) -> Option<String> {
    let (place, words) = name.split_at_checked(2)?;
    let words = words.strip_prefix('-')?;
    let well_formed = place.bytes().all(|byte| byte.is_ascii_digit())
        && words
            .split('-')
            .all(|word| !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase()));
    well_formed.then(|| words.replace('-', " ").to_ascii_uppercase())
}

/// A text's lines, grouped as the page lays them out.
pub fn blocks(text: &str, kind: Kind) -> Vec<Block<'_>> {
    let mut blocks: Vec<Block<'_>> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        match (blocks.last_mut(), line) {
            (Some(Block::Gap), "") => {}
            (_, "") => blocks.push(Block::Gap),
            (Some(Block::Kept(lines)), line) if keeps_its_layout(kind, index, line) => {
                lines.push(line)
            }
            (_, line) if keeps_its_layout(kind, index, line) => {
                blocks.push(Block::Kept(vec![line]))
            }
            (Some(Block::Prose(lines)), line) => lines.push(line),
            (_, line) => blocks.push(Block::Prose(vec![line])),
        }
    }
    blocks
}

/// The lines of a text that keep their layout and are wider than `KEPT_WIDTH`, as their
/// line numbers, counted from 1, and their widths.
pub fn too_wide(text: &str, kind: Kind) -> Vec<(usize, usize)> {
    text.lines()
        .enumerate()
        .filter(|&(index, line)| keeps_its_layout(kind, index, line) && line.len() > KEPT_WIDTH)
        .map(|(index, line)| (index + 1, line.len()))
        .collect()
}

/// Whether the line at `index`, counted from 0, of a text keeps its layout.
fn keeps_its_layout(kind: Kind, index: usize, line: &str) -> bool {
    (kind == Kind::Help && index == 0) || line.starts_with(' ')
}

fn section(page: &mut String, heading: &str, text: &str, kind: Kind) {
    // The heading is quoted so that it is one argument, and escaped as a text line is.
    page.push_str(&format!(".SH \"{}\"\n", escaped(heading)));
    for block in blocks(text, kind) {
        match block {
            Block::Kept(lines) => {
                page.push_str(".nf\n");
                for line in lines {
                    page.push_str(&escaped(line));
                    page.push('\n');
                }
                page.push_str(".fi\n");
            }
            Block::Prose(lines) => {
                for line in lines {
                    page.push_str(&escaped(line));
                    page.push('\n');
                }
            }
            Block::Gap => page.push_str(".PP\n"),
        }
    }
}

/// A line of a text as `man` must be given it to show it byte for byte: `\` and `-` are
/// markup unless escaped, and a line beginning with `.` or `'` is a request.
fn escaped(line: &str) -> String {
    let mut out = String::new();
    if line.starts_with(['.', '\'']) {
        out.push_str("\\&");
    }
    for character in line.chars() {
        match character {
            '\\' => out.push_str("\\e"),
            '-' => out.push_str("\\-"),
            other => out.push(other),
        }
    }
    out
}
