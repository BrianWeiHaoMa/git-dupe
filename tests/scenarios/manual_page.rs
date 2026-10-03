//! The manual page the build renders from the help texts and from the page's own texts:
//! every text present is in it, under its section's heading and laid out as the renderer
//! says — each line that keeps its layout as written, as `git dupe help` prints it, prose
//! filled to the terminal's width with every word whole and in order — the page's own
//! texts between the description and the commands' sections, and this machine's `man`
//! shows no line of it wider than 80 columns on a terminal of 80, both for the page read
//! as a local file and for the page installed into a manual path, where `man git-dupe`
//! and `git dupe --help` find it. What the page's own texts say is run by
//! `manual_page_commands`, and what its explanations state is checked here by the words
//! that state it.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::manpage::Kind;
use crate::harness::{
    End, FreshDirectory, TERMINAL, asking_for_the_page, every_text_shown,
    holds_a_line_of_the_general_text, lines_of, manpage, page_texts_present, shown_by_man,
    shown_under, text_present, texts_present, under_each_release,
};

/// The page as the build script wrote it for this build.
fn built_page() -> PathBuf {
    Path::new(env!("OUT_DIR")).join("git-dupe.1")
}

/// What `man` shows of `page` on a terminal `columns` wide.
fn shown(page: &str, columns: usize) -> Vec<String> {
    let directory = FreshDirectory::create();
    let file = directory.path().join("git-dupe.1");
    fs::write(&file, page).unwrap();
    shown_by_man(&file, columns)
}

/// The lines of `shown` wider than `columns`.
fn wider_than(shown: &[String], columns: usize) -> Vec<&str> {
    shown
        .iter()
        .map(String::as_str)
        .filter(|line| line.chars().count() > columns)
        .collect()
}

/// The page of one command `sample` with `text`, beside a general text of one line.
fn sample_page(text: &str) -> String {
    manpage::render(
        "usage: git dupe\n",
        &[],
        &[("sample".to_owned(), text.to_owned())],
    )
}

#[test]
fn the_built_page_shows_every_text_present_within_80_columns() {
    let narrow = shown_by_man(&built_page(), TERMINAL);
    every_text_shown(&narrow, "`man -l` at 80 columns");
    assert_eq!(wider_than(&narrow, TERMINAL), Vec::<&str>::new());

    let wide = shown_by_man(&built_page(), 120);
    every_text_shown(&wide, "`man -l` at 120 columns");
    // No line of a text is wider than 80 columns, nor than 87 with its indentation: a
    // wider line is prose that `man` filled to the wider terminal.
    assert!(
        !wider_than(&wide, 87).is_empty(),
        "no prose filled to 120 columns:\n{}",
        wide.join("\n")
    );
}

#[test]
fn the_page_s_own_texts_stand_between_the_description_and_the_commands() {
    let mut commands: Vec<String> = texts_present()
        .iter()
        .filter_map(|(path, _)| path.file_stem().and_then(|stem| stem.to_str()))
        .filter(|&stem| stem != "general")
        .map(manpage::heading)
        .collect();
    commands.sort();
    let mut expected = vec!["NAME".to_owned(), manpage::DESCRIPTION.to_owned()];
    expected.extend(
        page_texts_present()
            .into_iter()
            .map(|(_, heading, _)| heading),
    );
    expected.extend(commands);
    assert!(
        expected[2..4] == ["QUICK START", "EXAMPLES"],
        "the quick start and the examples come first: {expected:?}"
    );

    // A heading is the one kind of line `man` shows at the left margin, beside the title
    // line above the page and the footer below it.
    let shown = shown_by_man(&built_page(), TERMINAL);
    let at_the_margin: Vec<&str> = shown
        .iter()
        .map(String::as_str)
        .filter(|line| !line.is_empty() && !line.starts_with(' '))
        .collect();
    let headings = &at_the_margin[1..];
    assert_eq!(headings, expected, "{}", shown.join("\n"));
}

#[test]
fn a_page_text_has_no_usage_line_and_keeps_its_indented_lines_as_written() {
    // A first line of prose long enough that `man` fills it over two lines, a command
    // line, and a line as wide as a line that keeps its layout may be.
    let text = format!(
        "A first line of prose, which a help text would keep as its usage line, filled.\n\
         \n\
         \x20   $ git dupe commit -m \"Private files\"\n\
         \x20 {}\n",
        "r".repeat(manpage::KEPT_WIDTH - 2)
    );
    let page = manpage::render(
        "usage: git dupe\n",
        &[("SAMPLE TEXT".to_owned(), text.clone())],
        &[("sample".to_owned(), "usage: git dupe sample\n".to_owned())],
    );
    let shown = shown(&page, TERMINAL);
    assert_eq!(
        shown_under(&shown, "SAMPLE TEXT", &text, Kind::Page),
        Ok(()),
        "{}",
        shown.join("\n")
    );
    assert!(shown_under(&shown, "SAMPLE TEXT", &text, Kind::Help).is_err());
    assert_eq!(wider_than(&shown, TERMINAL), Vec::<&str>::new());

    // Its first line is prose at any width; its indented lines are held to the width of
    // a line that keeps its layout.
    let wide = format!(
        "{}\n  {}\n",
        "p".repeat(90),
        "r".repeat(manpage::KEPT_WIDTH - 1)
    );
    assert_eq!(
        manpage::too_wide(&wide, Kind::Page),
        vec![(2, manpage::KEPT_WIDTH + 1)]
    );
    assert_eq!(
        manpage::too_wide(&wide, Kind::Help),
        vec![(1, 90), (2, manpage::KEPT_WIDTH + 1)]
    );
}

#[test]
fn a_page_text_takes_its_place_and_heading_from_its_file_name() {
    for (name, heading) in [
        ("10-quick-start", Some("QUICK START")),
        ("20-examples", Some("EXAMPLES")),
        (
            "05-why-a-second-repository",
            Some("WHY A SECOND REPOSITORY"),
        ),
        ("quick-start", None),
        ("1-quick-start", None),
        ("100-quick-start", None),
        ("10quick-start", None),
        ("10-", None),
        ("10-Quick-start", None),
        ("10-quick--start", None),
        ("10-quick-start-", None),
        ("10-quick_start", None),
        ("ab-quick-start", None),
    ] {
        assert_eq!(manpage::page_heading(name).as_deref(), heading, "{name}");
    }
}

#[test]
fn the_installed_page_is_what_man_git_dupe_and_git_dupe_help_show() {
    let page = fs::read(built_page()).unwrap();
    under_each_release(|s| {
        // Two manual paths: one holding exactly the page this build rendered, and one
        // whose `man1` holds nothing. The second is what tells the installed page from a
        // page found anywhere else on this machine.
        let installed = s.dir().join("installed");
        let empty = s.dir().join("empty");
        fs::create_dir_all(installed.join("man1")).unwrap();
        fs::create_dir_all(empty.join("man1")).unwrap();
        fs::write(installed.join("man1/git-dupe.1"), &page).unwrap();

        let [man, help] = asking_for_the_page(s, &installed).map(|asked| asked.run());
        assert_eq!(man.end, End::Code(0), "{man:?}");
        let shown = lines_of(&man.stdout);
        every_text_shown(&shown, "`man git-dupe`");
        assert_eq!(wider_than(&shown, TERMINAL), Vec::<&str>::new());
        assert_eq!(help.end, End::Code(0), "{help:?}");
        assert_eq!(help.stdout, man.stdout, "`git dupe --help`: {help:?}");

        for absent in asking_for_the_page(s, &empty).map(|asked| asked.run()) {
            assert_ne!(absent.end, End::Code(0), "{absent:?}");
            assert!(
                !holds_a_line_of_the_general_text(&absent.stdout),
                "{absent:?}"
            );
        }
        // Neither of these needs the page, wherever the manual path points.
        for (words, text) in [
            (&["dupe", "help"][..], "general"),
            (&["dupe", "add", "-h"], "add"),
        ] {
            let answer = s.git(words).variable("MANPATH", &empty).run();
            assert_eq!(answer.end, End::Code(0), "{words:?}: {answer:?}");
            assert_eq!(answer.stdout, text_present(text).as_bytes(), "{words:?}");
        }
    });
}

#[test]
fn every_hyphen_of_the_built_page_is_escaped() {
    // `man` here shows an unescaped `-` intact, so only the page's source can tell: left
    // unescaped, an option's dashes are typographic hyphens to another `man`.
    let page = fs::read_to_string(built_page()).unwrap();
    let hyphens = page.matches('-').count();
    assert_eq!(page.matches("\\-").count(), hyphens);
    let in_the_texts: usize = texts_present()
        .into_iter()
        .map(|(_, text)| text)
        .chain(page_texts_present().into_iter().map(|(_, _, text)| text))
        .map(|text| text.matches('-').count())
        .sum();
    assert!(hyphens >= in_the_texts && in_the_texts > 0);
}

#[test]
fn lines_that_are_markup_to_man_are_shown_as_written() {
    let text = "usage: git dupe sample [-n | --dry-run] <path>\n\
                \n\
                .gitdupe begins with a dot\n\
                'quoted' begins with a quote\n\
                \x20   -n, --dry-run    an indented option line\n\
                \x20   \\n and \\fB in a kept line, and a trailing \\\n\
                a backslash \\n and \\fB stay as typed\\\n\
                ... and an ellipsis\n";
    let general = "usage: git dupe <command>\n\n.first and 'second\n'third and .fourth\n";
    let page = manpage::render(general, &[], &[("sample".to_owned(), text.to_owned())]);
    let shown = shown(&page, TERMINAL);
    for (heading, text) in [
        (manpage::DESCRIPTION.to_owned(), general),
        (manpage::heading("sample"), text),
    ] {
        if let Err(why) = shown_under(&shown, &heading, text, Kind::Help) {
            panic!("{why}\n{page}\nis shown as\n{}", shown.join("\n"));
        }
    }
}

#[test]
fn prose_and_the_lines_that_keep_their_layout_each_start_a_line_of_their_own() {
    let text = "usage: git dupe sample <path>\n\
                Prose right below the usage line, long enough to fill more than one line of\n\
                the page on a terminal of 80 columns, which it does once it runs on a while.\n\
                Options, right above a table:\n\
                \x20 -a, --all      -b, --bee\n\
                \x20 -c, --sea\n\
                Prose right below the table.\n\
                \n\
                A second paragraph, after an empty line.\n";
    let heading = manpage::heading("sample");
    let page = sample_page(text);
    for columns in [TERMINAL, 120] {
        let shown = shown(&page, columns);
        if let Err(why) = shown_under(&shown, &heading, text, Kind::Help) {
            panic!("{why}\n{page}\nis shown as\n{}", shown.join("\n"));
        }
    }

    // A page that loses the layout keeps every word and still is not the text.
    for (lost, from, to) in [
        (
            "the usage line filled with the prose below it",
            ".nf\nusage: git dupe sample <path>\n.fi\n",
            "usage: git dupe sample <path>\n",
        ),
        (
            "the table filled with the prose around it",
            ".nf\n  \\-a, \\-\\-all      \\-b, \\-\\-bee\n  \\-c, \\-\\-sea\n.fi\n",
            "\\-a, \\-\\-all      \\-b, \\-\\-bee\n\\-c, \\-\\-sea\n",
        ),
        ("the two paragraphs run into one", ".PP\n", ""),
        (
            "a line of prose dropped",
            "Options, right above a table:\n",
            "",
        ),
    ] {
        let wrong = page.replacen(from, to, 1);
        assert_ne!(wrong, page, "{lost}: {from:?} is not in\n{page}");
        let shown = shown(&wrong, TERMINAL);
        assert!(
            shown_under(&shown, &heading, text, Kind::Help).is_err(),
            "{lost}, and the check still finds the text:\n{}",
            shown.join("\n")
        );
    }
}

#[test]
fn prose_is_filled_with_every_word_whole_and_single_spaces_between_words() {
    // Words a terminal of 80 columns tempts `man` to break or space out: option names,
    // hyphenated words, paths, quoted commands, a long path, and a URL.
    let text = "usage: git dupe sample\n\
                git-dupe versions private files in .git/dupe, a second repository whose\n\
                working tree is the project's; Git's own commands run on it. A hidden path,\n\
                which the project's Git ignores, is .gitdupe, a path it lists, or a\n\
                privately tracked file. While a commit on a private branch, tag, or stash\n\
                entry is on no remote-tracking branch, detach refuses. The options\n\
                --no-ignore-removal and --untracked-files[=<mode>] and --pathspec-from-file\n\
                are words like any other; so is one-file-at-a-time. \"git dupe git add ...\"\n\
                runs any of them as Git's own add, unguarded. It reads .git/info/exclude,\n\
                a/very/long/path/with/many/slashes/that/could/tempt/a/break/here.txt and\n\
                https://mirrors.edge.kernel.org/pub/software/scm/git/git-2.56.0.tar.xz as\n\
                words.\n";
    let heading = manpage::heading("sample");
    let page = sample_page(text);
    for columns in [TERMINAL, 120] {
        let shown = shown(&page, columns);
        if let Err(why) = shown_under(&shown, &heading, text, Kind::Help) {
            panic!("{why}\n{page}\nis shown as\n{}", shown.join("\n"));
        }
        assert_eq!(wider_than(&shown, columns), Vec::<&str>::new());
        // Two spaces stand only after a sentence's end, where `man` puts them; no line
        // is padded between its words to reach the margin.
        let sentence_end = |before: &str| {
            before
                .trim_end_matches(['"', '\'', ')', ']', '*'])
                .ends_with(['.', '?', '!'])
        };
        let padded: Vec<&String> = shown
            .iter()
            .filter(|line| {
                let words = line.trim_start();
                line.len() - words.len() == 7
                    && (words.contains("   ")
                        || words
                            .match_indices("  ")
                            .any(|(at, _)| !sentence_end(&words[..at])))
            })
            .collect();
        assert!(padded.is_empty(), "{padded:#?}");
    }
}

#[test]
fn a_line_that_keeps_its_layout_fits_80_columns_at_73_columns_and_not_at_74() {
    let prose = "A line of prose as wide as a help text allows any of its lines; man fills it in.";
    assert_eq!(prose.len(), 80);
    for width in [73, 74] {
        let usage = format!("usage: git dupe sample {}", "u".repeat(width - 23));
        let row = format!("  {}", "r".repeat(width - 2));
        let text = format!("{usage}\n\n{prose}\n{row}\n");
        let wide = if width > manpage::KEPT_WIDTH {
            vec![(1, width), (4, width)]
        } else {
            vec![]
        };
        assert_eq!(manpage::too_wide(&text, Kind::Help), wide, "{text}");

        let shown = shown(&sample_page(&text), TERMINAL);
        if let Err(why) = shown_under(&shown, &manpage::heading("sample"), &text, Kind::Help) {
            panic!("{why}\n{}", shown.join("\n"));
        }
        let over: Vec<usize> = wider_than(&shown, TERMINAL)
            .iter()
            .map(|line| line.len())
            .collect();
        assert_eq!(over, vec![7 + width; wide.len()], "{}", shown.join("\n"));
    }
}

#[test]
fn a_word_broken_across_two_lines_is_not_the_text() {
    let text = "usage: git dupe sample\n--pathspec-from-file is one word\n";
    let as_shown = |lines: &[&str]| -> Vec<String> {
        ["GIT DUPE SAMPLE", "       usage: git dupe sample"]
            .iter()
            .chain(lines)
            .map(|line| (*line).to_owned())
            .collect()
    };
    let whole = as_shown(&["       --pathspec-from-file is one word"]);
    assert_eq!(
        shown_under(&whole, "GIT DUPE SAMPLE", text, Kind::Help),
        Ok(())
    );
    let broken = as_shown(&["       --pathspec-", "       from-file is one word"]);
    assert!(shown_under(&broken, "GIT DUPE SAMPLE", text, Kind::Help).is_err());
}

#[test]
fn the_page_is_the_same_bytes_for_the_same_texts() {
    let texts = [("help".to_owned(), "usage: git dupe help\n".to_owned())];
    let page_texts = [("SAMPLE".to_owned(), "Prose.\n".to_owned())];
    let once = manpage::render("usage: git dupe\n", &page_texts, &texts);
    assert_eq!(
        once,
        manpage::render("usage: git dupe\n", &page_texts, &texts)
    );
    assert!(once.starts_with(".TH "), "{once}");
}

/// The explanations the screen of `git dupe help` has no room for (F10), each a section of
/// the page, by the words that state them read across their line breaks: the detail the
/// general text sheds, of the refusals of G17 and G18 above all, stands here.
#[test]
fn the_page_explains_what_the_general_text_has_no_room_for() {
    let explanations: [(&str, &[&str]); 4] = [
        (
            "HIDDEN AND PRIVATE",
            &[
                "A hidden path is one the project's Git ignores",
                "A private file is one the private repository tracks.",
                "Every private file is hidden, but being hidden versions nothing.",
                "git dupe hide scratch/ lists scratch in .gitdupe and stages .gitdupe",
                "nothing at it is staged",
                "never a file only the project's Git tracks, and a file the project ignores only with -f",
                "the files under it that the private repository tracks stay hidden",
                "names git dupe hide, run from the root, where that command would hide it again",
                "A path is tracked by one repository or the other.",
                "git dupe add never stages a file only the project's Git tracks",
                "git rm --cached README.md, which stages its deletion from the project, a deletion every other clone receives once it is committed and pushed, and then git dupe add README.md, with -f where the project ignores it, which tracks it privately.",
            ],
        ),
        (
            "FILES THE PROJECT IGNORES",
            &[
                "is added only with -f",
                "The -f is needed once per file.",
                "git dupe add ., git dupe add -A, git dupe add -u, and git dupe commit -a take its changes without -f",
                "A new file the project ignores needs -f again",
            ],
        ),
        (
            "PLAIN GIT",
            &[
                "Plain git is not guarded",
                "git clean -x deletes them",
                "git stash -a stashes them, and removes them from disk",
                "git add -f stages them in the project",
                "a pull or checkout overwrites one",
                "overwrites the private file without a word",
                "Both repositories then track the path, and every git dupe command warns of it",
                "brings back every privately tracked file as last staged",
                "An edit never staged is gone.",
                "Commit private work before a public pull or checkout, and before git clean -x.",
            ],
        ),
        (
            "REFUSED COMMANDS",
            &[
                "-u, --include-untracked, -a, or --all",
                "with git dupe add, then run git dupe stash",
                "git dupe push, pull, fetch, remote, and clone are refused",
                "names the project's repository",
                "a URL the project's repository gives one of its remotes",
                "a working tree of the project",
                "a URL that the private repository's configuration gives any of its remotes",
                "naming that remote and git dupe remote remove",
                "the default remote",
                "git dupe git for a destination you mean",
                "This is a comparison of names, not a proof that a destination is private",
            ],
        ),
    ];
    let texts = page_texts_present();
    for (heading, phrases) in explanations {
        let Some((_, _, text)) = texts.iter().find(|(_, found, _)| found == heading) else {
            panic!("no page text {heading}");
        };
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in phrases {
            assert!(text.contains(phrase), "{heading}: missing {phrase}: {text}");
        }
    }
}
