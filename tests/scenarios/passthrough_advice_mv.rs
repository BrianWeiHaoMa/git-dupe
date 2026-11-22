//! The hint after a private `mv` of a directory `.gitdupe` lists (`Holds/G19`, G25, G11): Git
//! moves the directory and prints nothing, and `.gitdupe` goes on listing the old name, so
//! that a new file under the new name would be visible to public Git. One `hint:` follows
//! Git's output, naming `git dupe hide -- <new>` and then `git dupe unhide -- <old>`, run
//! from the root; run as written, in that order, they leave a new file there hidden and
//! untracked, and the old name unlisted. Where the destination stood as a directory, the
//! new name is the one Git moved the source to inside it. None follows a `mv` of a file no
//! line lists, a `mv` inside a listed directory, a dry run, a failed run, or a help request,
//! and none a `git dupe git mv`. A path `hide` would refuse as a pattern gets the edit of
//! `.gitdupe` instead of a command, and every command a hint names is one `hide` and
//! `unhide` accept. The list is read through the keeper alone, with no Git run while
//! `.gitdupe` stands on disk (G22).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, asked_after, dupe_succeeds, hinted, private_add, private_alias,
    private_commit, under_each_release, unhinted, write,
};

/// The attached workspace `name` whose `.gitdupe` lists `notes`, holding a committed `notes/saved.md`
/// and an untracked `notes/draft.md`, and whose private repository also tracks a
/// `todo.txt` no line lists.
fn fixture(s: &Scenario, name: &str) -> PathBuf {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let root = s.dir().join(name);
    s.repository(&root);
    s.init(&root);
    write(&root, "notes/saved.md", b"saved\n");
    dupe_succeeds(s, &root, &["add", "notes/"]);
    write(&root, "todo.txt", b"todo\n");
    private_add(s, &root, "todo.txt");
    private_commit(s, &root);
    write(&root, "notes/draft.md", b"draft\n");
    root
}

/// The commands the hint names, each between single quotes, in order.
fn named_commands(hint: &[u8]) -> Vec<String> {
    let text = String::from_utf8(hint.to_vec()).unwrap();
    text.split('\'')
        .skip(1)
        .step_by(2)
        .filter(|quoted| quoted.starts_with("git dupe "))
        .map(str::to_string)
        .collect()
}

/// Runs each of `commands` through the scenario's shell from `root`, as the hint shows
/// it: each must exit 0.
fn run_as_written(s: &Scenario, root: &Path, commands: &[String]) {
    for command in commands {
        let ran = s.program(s.shell(), ["-c", command]).from(root).run();
        assert_eq!(ran.end, End::Code(0), "{command}: {ran:?}");
    }
}

/// The lines of `.gitdupe` at `root`.
fn gitdupe_lines(root: &Path) -> Vec<String> {
    let read = fs::read(root.join(".gitdupe")).unwrap();
    String::from_utf8(read)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

/// The hint line of `output`, its last line of standard error.
fn hint_line(output: &Output) -> Vec<u8> {
    let stderr = output.stderr.strip_suffix(b"\n").unwrap_or(&output.stderr);
    let line = stderr.rsplit(|byte| *byte == b'\n').next().unwrap();
    line.strip_prefix(b"hint: ").unwrap_or(line).to_vec()
}

/// Whether public `git status` names anything under `path`.
fn publicly_seen(s: &Scenario, root: &Path, path: &str) -> bool {
    let status = s
        .git(["status", "--porcelain", "-uall"])
        .from(root)
        .succeeds();
    String::from_utf8(status.stdout)
        .unwrap()
        .lines()
        .any(|line| line[3..].starts_with(path))
}

#[test]
fn a_listed_directory_moved_ends_with_one_hint_whose_commands_hide_the_new_name() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-1");
        let moved = hinted(
            s,
            &root,
            &["mv", "notes", "journal"],
            &[
                b"'git dupe hide -- journal'",
                b"'git dupe unhide -- notes'",
                b"run from the root",
            ],
        );
        assert_eq!(moved.end, End::Code(0), "{moved:?}");
        let commands = named_commands(&hint_line(&moved));
        assert_eq!(
            commands,
            ["git dupe hide -- journal", "git dupe unhide -- notes"]
        );
        assert!(publicly_seen(s, &root, "journal/draft.md"));
        run_as_written(s, &root, &commands);
        write(&root, "journal/new.md", b"new\n");
        assert!(!publicly_seen(s, &root, "journal/"));
        let private = dupe_succeeds(s, &root, &["status", "--porcelain"]);
        let private = String::from_utf8(private.stdout).unwrap();
        assert!(private.contains("?? journal/"), "{private}");
        assert_eq!(gitdupe_lines(&root), ["journal"]);
    });
}

#[test]
fn moved_into_a_directory_that_stood_the_hint_names_where_it_now_stands() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-2");
        write(&root, "box/kept.md", b"kept\n");
        private_alias(s, &root, "rename", "mv -v");
        let moved = hinted(
            s,
            &root,
            &["rename", "notes", "box"],
            &[
                b"'git dupe hide -- box/notes'",
                b"'git dupe unhide -- notes'",
            ],
        );
        assert_eq!(moved.end, End::Code(0), "{moved:?}");
        run_as_written(s, &root, &named_commands(&hint_line(&moved)));
        write(&root, "box/notes/new.md", b"new\n");
        assert!(!publicly_seen(s, &root, "box/notes/"));
        assert_eq!(gitdupe_lines(&root), ["box/notes"]);
    });
}

#[test]
fn every_command_a_hint_names_is_one_hide_and_unhide_accept() {
    under_each_release(|s| {
        // Two listed sources into one directory, one of them beginning with `:`.
        let root = fixture(s, "workspace-3");
        write(&root, ":plans/p.md", b"p\n");
        dupe_succeeds(s, &root, &["add", "--", "./:plans/"]);
        write(&root, "box/kept.md", b"kept\n");
        let moved = hinted(
            s,
            &root,
            &["mv", "notes", ":plans", "box"],
            &[
                b"'git dupe hide -- box/notes box/:plans'",
                b"'git dupe unhide -- notes ./:plans'",
            ],
        );
        run_as_written(s, &root, &named_commands(&hint_line(&moved)));
        assert_eq!(gitdupe_lines(&root), ["box/notes", "box/:plans"]);

        // A `mv -k` that skipped a source it could not move: only what moved is named.
        let root = fixture(s, "workspace-4");
        write(&root, "plans/p.md", b"p\n");
        dupe_succeeds(s, &root, &["add", "plans/"]);
        write(&root, "box/kept.md", b"kept\n");
        let moved = hinted(
            s,
            &root,
            &["mv", "-k", "notes", "missing", "box"],
            &[
                b"'git dupe hide -- box/notes'",
                b"'git dupe unhide -- notes'",
            ],
        );
        run_as_written(s, &root, &named_commands(&hint_line(&moved)));

        // A destination holding `[`: the edit of `.gitdupe`, and no command `hide` refuses.
        let root = fixture(s, "workspace-5");
        let moved = hinted(
            s,
            &root,
            &["mv", "notes", "journal[1]"],
            &[b"edit .gitdupe so that it lists journal[1] in place of notes"],
        );
        let commands = named_commands(&hint_line(&moved));
        assert_eq!(commands, ["git dupe add -- .gitdupe"]);
    });
}

#[test]
fn no_hint_follows_a_move_that_left_nothing_listed_unhidden() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-6");
        // A privately tracked file no line lists stays hidden by being tracked.
        let file = unhinted(s, &root, &["mv", "todo.txt", "done.txt"]);
        assert_eq!(file.end, End::Code(0), "{file:?}");
        // Inside a listed directory, the listing still hides it.
        let inside = unhinted(s, &root, &["mv", "notes/saved.md", "notes/kept.md"]);
        assert_eq!(inside.end, End::Code(0), "{inside:?}");
        // A directory below a listed one is no line of `.gitdupe`: no listing moves with
        // it, and no `unhide` of it is named.
        write(&root, "notes/nested/n.md", b"n\n");
        dupe_succeeds(s, &root, &["add", "notes/nested/n.md"]);
        let below = unhinted(s, &root, &["mv", "notes/nested", "elsewhere"]);
        assert_eq!(below.end, End::Code(0), "{below:?}");
        // A dry run moves nothing.
        for words in [
            &["mv", "-n", "notes", "journal"][..],
            &["mv", "--dry-run", "notes", "journal"],
            &["mv", "-vn", "notes", "journal"],
        ] {
            let dry = unhinted(s, &root, words);
            assert_eq!(dry.end, End::Code(0), "{dry:?}");
        }
        // A failed run, and a help request.
        let failed = unhinted(s, &root, &["mv", "nosuch", "journal"]);
        assert_ne!(failed.end, End::Code(0), "{failed:?}");
        unhinted(s, &root, &["mv", "-h"]);
        // Into a directory another line hides, it stays hidden.
        write(&root, "archive/old.md", b"old\n");
        dupe_succeeds(s, &root, &["hide", "archive/"]);
        let hidden = unhinted(s, &root, &["mv", "notes", "archive"]);
        assert_eq!(hidden.end, End::Code(0), "{hidden:?}");
    });
}

#[test]
fn a_dry_run_undone_by_no_dry_run_moves_and_gets_the_hint() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-8");
        let moved = hinted(
            s,
            &root,
            &["mv", "-n", "--no-dry-run", "notes", "journal"],
            &[b"'git dupe hide -- journal'", b"'git dupe unhide -- notes'"],
        );
        assert_eq!(moved.end, End::Code(0), "{moved:?}");
    });
}

#[test]
fn a_source_mv_k_skipped_is_not_named_beside_a_place_that_stood_already() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-9");
        // `notes` moved aside by hand, `.gitdupe` still listing it, and a `box/notes` of
        // its own already standing: Git skips the absent source and moves nothing.
        fs::rename(root.join("notes"), root.join("aside")).unwrap();
        write(&root, "box/notes/own.md", b"own\n");
        let skipped = unhinted(s, &root, &["mv", "-k", "notes", "box"]);
        assert_eq!(skipped.end, End::Code(0), "{skipped:?}");
    });
}

#[test]
fn a_new_place_under_which_the_project_tracks_a_path_is_offered_the_move_back() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-10");
        // The project tracks `journal/draft.md`, gone from disk, so that `hide` refuses
        // `journal` once the private draft stands there.
        write(&root, "journal/draft.md", b"the project's\n");
        s.git(["add", "journal/draft.md"]).from(&root).succeeds();
        s.commit_public(&root);
        fs::remove_dir_all(root.join("journal")).unwrap();
        let moved = hinted(
            s,
            &root,
            &["mv", "notes", "journal"],
            &[b"'git dupe mv -- journal notes'", b"run from the root"],
        );
        assert_eq!(moved.end, End::Code(0), "{moved:?}");
        let commands = named_commands(&hint_line(&moved));
        assert_eq!(commands, ["git dupe mv -- journal notes"]);
        run_as_written(s, &root, &commands);
        assert_eq!(fs::read(root.join("notes/draft.md")).unwrap(), b"draft\n");
        assert!(!publicly_seen(s, &root, "notes/"));
    });
}

#[test]
fn the_list_costs_no_git_run_while_gitdupe_stands_on_disk() {
    under_each_release(|s| {
        let root = fixture(s, "workspace-7");
        // The list costs none; the new place is asked of the public index once.
        let listing: &[&str] = &[
            "ls-files",
            "-z",
            "--full-name",
            "--",
            ":(top,literal)journal",
        ];
        asked_after(
            s,
            &root,
            &["mv", "notes", "journal"],
            End::Code(0),
            &[listing],
        );
    });
}
