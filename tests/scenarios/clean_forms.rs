//! `git dupe clean` under each of its forms, over a hidden path of every kind: every
//! spared path stays byte for byte, and everything else goes or stays as plain
//! `git clean` with the same words decides, under each release (G16, S4).

use std::fs;

use crate::harness::{
    EVERY_KIND_SPARED, End, Scenario, Twin, hidden_path_of_every_kind, under_each_release, write,
};

fn every_kind(s: &Scenario) -> Twin<'_> {
    let built = s.dir().join("workspace");
    hidden_path_of_every_kind(s, &built);
    Twin::of(s, &built, &EVERY_KIND_SPARED)
}

#[test]
fn every_form_without_x_spares_every_hidden_path_and_deletes_the_rest_as_git_does() {
    under_each_release(|s| {
        let twin = every_kind(s);
        for words in [
            &["-f"][..],
            &["-fd"],
            &["-fx"],
            &["-fdx"],
            &["-ffd"],
            &["-ffdx"],
            &["-n"],
            &["-nd"],
            &["-nx"],
            &["-ndx"],
            &["--force", "-d", "-x", "--quiet"],
            &["-q", "-f", "-d"],
            &["--dry-run", "-d"],
            // Without `-f`, `-n`, or `-i`: Git's own refusal, nothing deleted.
            &[],
            &["-d"],
            &["-x"],
            // The user's own patterns, in every spelling, stand before git-dupe's, which
            // decide: a negation of the user's spares nothing it would not.
            &["-fdx", "-e", "!/notes", "-e", "!/.env.local", "-e", "*.txt"],
            &["-fdx", "-e*.o"],
            &["-fdx", "--exclude=*.txt"],
            &["-fdx", "--exclude", "*.txt"],
            &["-fdxe", "*.o"],
        ] {
            twin.clean(words).run();
        }
    });
}

#[test]
fn under_x_an_ignored_directory_holding_a_hidden_path_is_entered_and_nothing_else_changes() {
    under_each_release(|s| {
        let twin = every_kind(s);
        // Plain Git keeps these four directories whole without `-d`; the command enters
        // them and deletes their ignored files, every hidden path staying (G16).
        twin.clean(&["-fX"])
            .admitting(&[
                "build/out.js",
                "ign/junk.txt",
                "ign/deep/junk2.txt",
                "ig *[d/junk.txt",
                "allign/a.o",
            ])
            .run();
        for words in [&["-fdX"][..], &["-ffdX"], &["-nX"], &["-ndX"]] {
            twin.clean(words).run();
        }
    });
}

#[test]
fn a_pathspec_with_or_without_the_end_of_options_spares_every_hidden_path() {
    under_each_release(|s| {
        let twin = every_kind(s);
        for words in [
            &["-f", "--", "notes", "scratch.txt"][..],
            &["-fx", "--", "notes", "build"],
            &["-fX", "--", "build", "ign"],
            &["-fdx", "--", "vendor", "fake", "dangling"],
            &["-fx", "--", "w*", "*.o", "cr*"],
            &["-fd", "--", "swapped-file", "swapped-dir", "absent"],
            &["-fx", "notes", "build"],
            &["-fX", "build", "ign"],
            &["-fdX", "notes"],
            &["-fX", "--", "notes"],
            &["-ndx", "--", "."],
        ] {
            twin.clean(words).run();
        }
    });
}

#[test]
fn from_a_directory_below_the_root_every_hidden_path_is_spared() {
    under_each_release(|s| {
        let twin = every_kind(s);
        twin.clean(&["-fdx"]).below("src").run();
        twin.clean(&["-fx"]).below("build").run();
        twin.clean(&["-fX"])
            .below("build")
            .admitting(&["build/out.js"])
            .run();
        twin.clean(&["-fX"]).below("notes").run();
        // From inside a directory plain Git removes whole: plain Git refuses to remove
        // the directory it runs in, and the command keeps it for the hidden path it holds.
        for (below, words) in [
            ("build", &["-fdX"][..]),
            ("notes", &["-fdx"]),
            ("notes", &["-fdX"]),
            ("vendor", &["-ffdx"]),
        ] {
            twin.clean(words).below(below).run();
            // The same directory named by `-C`: plain Git empties it and fails on `./`,
            // while the command keeps it and exits 0.
            twin.clean(words).named_by_c(below).plain_fails().run();
        }
    });
}

#[test]
fn interactively_and_without_force_required_every_hidden_path_is_spared() {
    under_each_release(|s| {
        let twin = every_kind(s);
        // `1` chooses Git's own "clean" in its menu.
        twin.clean(&["-idx"]).input(b"1\n").run();
        twin.clean(&["-iX"])
            .input(b"1\n")
            .admitting(&[
                "build/out.js",
                "ign/junk.txt",
                "ign/deep/junk2.txt",
                "ig *[d/junk.txt",
                "allign/a.o",
            ])
            .run();
        // Before 2.45.0 one `-f` under this setting deletes a nested repository whole, as
        // `-ff` does: plain Git says which under each release (S4).
        for words in [&["-d"][..], &["-dx"], &["-fd"], &["-fdx"], &["-dX"]] {
            twin.clean(words).setting("clean.requireForce=false").run();
        }
    });
}

/// Under `-X`, from a directory inside an ignored directory that holds a hidden path, or
/// with a pathspec inside one, plain Git takes that whole directory as one ignored entry,
/// while the command, which lifts it to spare the hidden path, reaches only what lies at
/// or below the user's directory and pathspecs: there it deletes the ignored files,
/// those plain Git keeps without `-d` included, and keeps the files of the directory
/// outside them, which plain Git deletes with it (G16).
#[test]
fn under_x_from_inside_an_ignored_directory_only_what_the_words_reach_is_deleted() {
    under_each_release(|s| {
        let twin = every_kind(s);
        let outside_deep = ["ign/junk.txt", "ign/other", "ign/other/z.txt"];
        twin.clean(&["-fdX"])
            .below("ign/deep")
            .keeping(&outside_deep)
            .run();
        twin.clean(&["-ffdX"])
            .below("ign/deep")
            .keeping(&outside_deep)
            .run();
        twin.clean(&["-fX"])
            .below("ign/deep")
            .admitting(&["ign/deep/junk2.txt"])
            .run();
        twin.clean(&["-fX", "--", "ign/junk.txt"])
            .keeping(&["ign/deep/junk2.txt", "ign/other", "ign/other/z.txt"])
            .run();
        twin.clean(&["-fdX", "--", "ign/deep"])
            .keeping(&outside_deep)
            .run();
        twin.clean(&["-fX", "ign/other", "build/out.js"])
            .keeping(&[
                "ign/junk.txt",
                "ign/deep/junk2.txt",
                "build/sub",
                "build/sub/x.o",
            ])
            .run();
    });
}

/// Under `-X`, the user's directory or a pathspec at or below an ignored directory beside
/// the hidden path's, within the ignored directory that holds the hidden path: plain Git
/// takes the whole ignored directory as one entry, while the command, which lifts the
/// directories that hold a hidden path, takes as one entry the outermost directory there
/// that holds none, removed with what it holds under `-d` or a pathspec and kept with it
/// otherwise, and keeps the rest of the ignored directory (G16, S4).
#[test]
fn under_x_an_ignored_directory_beside_the_hidden_paths_is_taken_whole_when_the_words_lie_in_it() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        s.attached_project(&built);
        write(&built, ".gitignore", b"ign/\n");
        s.git(["add", "--", ".gitignore"]).from(&built).succeeds();
        s.commit_public(&built);
        write(&built, ".gitdupe", b"ign/secret\nign/deep/keep\n");
        for path in [
            "ign/secret",
            "ign/junk.txt",
            "ign/other/outside",
            "ign/other/deeper/junk",
            "ign/deep/keep",
            "ign/deep/junk",
            "ign/deep/sib/junk",
            "scratch.txt",
        ] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        let twin = Twin::of(s, &built, &[".gitdupe", "ign/secret", "ign/deep/keep"]);

        // `ign/other` is the outermost directory in `ign` that holds no hidden path: taken
        // whole, `ign/other/outside` going with `ign/other/deeper/junk` where the words
        // reach only one of them, as all of `ign` goes for plain Git.
        let outside_other = [
            "ign/junk.txt",
            "ign/deep/junk",
            "ign/deep/sib",
            "ign/deep/sib/junk",
        ];
        for comparison in [
            twin.clean(&["-fX", "--", "ign/other/deeper/junk"]),
            twin.clean(&["-fX", "--", "ign/other/outside"]),
            twin.clean(&["-fdX", "--", "ign/other/deeper"]),
            twin.clean(&["-fdX"]).below("ign/other/deeper"),
        ] {
            let output = comparison.keeping(&outside_other).run();
            assert!(output.stderr.is_empty(), "{output:?}");
        }
        // Two levels in, the outermost such directory is `ign/deep/sib`, not `ign/deep`,
        // which holds a hidden path.
        let outside_sib = [
            "ign/junk.txt",
            "ign/other",
            "ign/other/outside",
            "ign/other/deeper",
            "ign/other/deeper/junk",
            "ign/deep/junk",
        ];
        for comparison in [
            twin.clean(&["-fX", "--", "ign/deep/sib/junk"]),
            twin.clean(&["-fdX"]).below("ign/deep/sib"),
        ] {
            let output = comparison.keeping(&outside_sib).run();
            assert!(output.stderr.is_empty(), "{output:?}");
        }
        // Without `-d` or a pathspec that directory is kept with what it holds, as plain
        // Git keeps all of `ign`.
        for below in ["ign/other/deeper", "ign/other", "ign/deep/sib"] {
            let output = twin.clean(&["-fX"]).below(below).run();
            assert!(output.stderr.is_empty(), "from {below:?}: {output:?}");
        }
    });
}

/// Under `-X`, a directory that holds a hidden path and that plain `git clean -X` takes as
/// ignored as a whole, where what stands at each hidden path inside it is a directory, an
/// untracked nested repository, or nothing: with `-d` or a pathspec the command enters it
/// and deletes its ignored files as plain Git with the same words reaches them, and every
/// hidden path stays with what stands at it. Without either, every entry of it is again
/// ignored or a nested repository, so Git takes it as one entry and keeps it whole, as
/// plain Git does; beside a hidden file, Git enters the directory that holds the file and
/// keeps whole the one that holds only a hidden directory and ignored files (G16, S4).
#[test]
fn under_x_a_directory_where_no_file_stands_at_a_hidden_path_is_one_entry_without_d() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        s.attached_project(&built);
        write(
            &built,
            ".gitignore",
            b"/ruled/\n/deep/\n*.o\n/nested/\n/absent/\n/empty/\n/mixed/\n",
        );
        s.git(["add", "--", ".gitignore"]).from(&built).succeeds();
        s.commit_public(&built);
        write(
            &built,
            ".gitdupe",
            b"ruled/private\ndeep/in/private\nall/private\nnested/repo/secret\n\
              absent/nothing\nempty/private\nmixed/a/file\nmixed/b/dir\n",
        );
        for path in [
            "ruled/private/secret",
            "ruled/junk",
            "deep/in/private/secret",
            "deep/in/junk",
            "deep/junk",
            "all/private/secret",
            "all/a.o",
            "nested/repo/secret",
            "nested/junk",
            "absent/junk",
            "empty/junk",
            "mixed/a/file",
            "mixed/a/junk",
            "mixed/b/dir/secret",
            "mixed/b/junk",
            "mixed/junk",
            "mixed/repo/file",
            "scratch.txt",
        ] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        fs::create_dir(built.join("empty/private")).unwrap();
        for repository in ["nested/repo", "mixed/repo"] {
            s.git(["init", "-q"])
                .from(&built.join(repository))
                .succeeds();
        }
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        let twin = Twin::of(
            s,
            &built,
            &[
                ".gitdupe",
                "ruled/private",
                "deep/in/private",
                "all/private",
                "nested/repo",
                "absent/nothing",
                "empty/private",
                "mixed/a/file",
                "mixed/b/dir",
            ],
        );

        twin.clean(&["-fdX"]).run();
        twin.clean(&["-ffdX"]).run();
        twin.clean(&["-fdX"])
            .below("deep/in")
            .keeping(&["deep/junk"])
            .run();
        twin.clean(&[
            "-fX",
            "--",
            "ruled/junk",
            "deep/junk",
            "all/a.o",
            "nested/junk",
            "absent/junk",
            "empty/junk",
        ])
        .keeping(&["deep/in/junk"])
        .run();
        twin.clean(&["-fdX"]).below("mixed").run();
        twin.clean(&["-fX", "--", "mixed/b/junk"])
            .keeping(&["mixed/junk", "mixed/a/junk"])
            .run();

        // Without `-d` or a pathspec, no file stands at a hidden path in `ruled`, `deep`,
        // `all`, `nested`, `absent`, `empty`, or `mixed/b`: each stays whole, from the
        // root and from inside it, as plain Git keeps it. `mixed/a/file` makes Git enter
        // `mixed/a` and `mixed`, and delete the ignored files there, the nested repository
        // `mixed/repo` and `mixed/b` each one entry that stays whole.
        let in_mixed = ["mixed/junk", "mixed/a/junk"];
        twin.clean(&["-fX"]).admitting(&in_mixed).run();
        twin.clean(&["-nX"]).run();
        twin.clean(&["-fX"])
            .below("mixed")
            .admitting(&in_mixed)
            .run();
        for below in ["ruled", "deep/in", "mixed/b"] {
            twin.clean(&["-fX"]).below(below).run();
        }
    });
}
