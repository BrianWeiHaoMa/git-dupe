//! `init` under a template directory of the developer's: one that holds a `HEAD`, one
//! whose files a kill left missing or partly copied, one whose `config` a kill left partly
//! copied, and one whose `config` includes a file.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, completed_hint, detached, dupe_succeeds, private_config_value,
    unchanged, under_each_release, write,
};

/// The template is copied as plain `git init` copies it, and its `HEAD` stands in place of
/// the branch `-b` or the public `HEAD` names.
#[test]
fn a_templates_head_stands_in_place_of_the_initial_branch() {
    under_each_release(|s| {
        let template = s.dir().join("template");
        fs::create_dir(&template).unwrap();
        fs::write(template.join("HEAD"), b"ref: refs/heads/trunk\n").unwrap();
        fs::write(template.join("description"), b"from the template\n").unwrap();
        let mut template_dir = OsString::from("init.templateDir=");
        template_dir.push(&template);
        for (index, (options, branch)) in [(&[][..], "main"), (&["-b", "feature"][..], "feature")]
            .into_iter()
            .enumerate()
        {
            let plain = s.dir().join(format!("plain-{index}"));
            let mut words = vec![OsStr::new("-c"), &template_dir, OsStr::new("init")];
            words.extend(options.iter().map(OsStr::new));
            words.extend([OsStr::new("-q"), plain.as_os_str()]);
            s.git(words).succeeds();

            let dir = s.dir().join(format!("workspace-{index}"));
            s.repository(&dir);
            let mut words = vec![OsStr::new("-c"), &template_dir, OsStr::new("dupe")];
            words.push(OsStr::new("init"));
            words.extend(options.iter().map(OsStr::new));
            let output = s.git(words).from(&dir).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.lines("fatal").is_empty(), "{output:?}");

            let head = fs::read(dir.join(".git/dupe/HEAD")).unwrap();
            assert_eq!(head, fs::read(plain.join(".git/HEAD")).unwrap());
            assert_ne!(head, format!("ref: refs/heads/{branch}\n").as_bytes());
            assert_eq!(
                fs::read(dir.join(".git/dupe/description")).unwrap(),
                fs::read(plain.join(".git/description")).unwrap()
            );
            let private = s
                .git(["dupe", "symbolic-ref", "HEAD"])
                .from(&dir)
                .succeeds();
            let public = s.git(["symbolic-ref", "HEAD"]).from(&plain).succeeds();
            assert_eq!(private.stdout, public.stdout);
            let worktree = s
                .private(&dir)
                .git(["config", "--local", "--get", "core.worktree"])
                .succeeds();
            assert_eq!(worktree.stdout, b"../..\n");
        }
    });
}

/// A kill among Git's copies of a template that holds a `HEAD`, which can be the first
/// file Git copies, leaves a repository Git cannot open with `HEAD` and some of the files.
/// Run again, `init` completes it under the same template: every file the kill left missing
/// is copied, those it left are kept, a partly copied one as it is, and so is a key of the
/// `config`, and the workspace ends configured as F2 says (`Holds/G1`, G21, S6).
#[test]
fn a_rerun_copies_the_template_files_a_kill_left_missing_and_keeps_those_it_left() {
    under_each_release(|s| {
        let names: Vec<String> = (0..20).map(|index| format!("file-{index:02}")).collect();
        let whole = |name: &str| format!("the whole of {name}\n").into_bytes();
        let contents: Vec<Vec<u8>> = names.iter().map(|name| whole(name)).collect();
        let mut files: Vec<(&str, &[u8])> = names
            .iter()
            .map(String::as_str)
            .zip(contents.iter().map(Vec::as_slice))
            .collect();
        files.push(("HEAD", b"ref: refs/heads/trunk\n"));
        let template_dir = template(s, &files);

        let dir = s.dir().join("killed");
        s.repository(&dir);
        let private = dir.join(".git/dupe");
        fs::create_dir(&private).unwrap();
        fs::write(private.join("HEAD"), b"ref: refs/heads/trunk\n").unwrap();
        fs::write(private.join("config"), b"[color]\n\tui = never\n").unwrap();
        fs::write(private.join("file-00"), b"the whole").unwrap();
        fs::write(private.join("file-01"), whole("file-01")).unwrap();

        let output = init_under(s, &template_dir, &dir);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        // Git's line for the repository it completed, and the hint saying so (G1).
        assert!(!output.stdout.is_empty(), "{output:?}");
        assert_eq!(output.lines("hint").len(), 1, "{output:?}");
        assert_eq!(fs::read(private.join("file-00")).unwrap(), b"the whole");
        for name in &names[1..] {
            assert_eq!(fs::read(private.join(name)).unwrap(), whole(name), "{name}");
        }
        assert!(private.join("objects").is_dir() && private.join("refs").is_dir());
        assert_eq!(
            fs::read(private.join("HEAD")).unwrap(),
            b"ref: refs/heads/trunk\n"
        );
        for (key, expected) in [
            ("color.ui", "never"),
            ("core.worktree", "../.."),
            ("status.showUntrackedFiles", "no"),
            ("advice.statusHints", "false"),
        ] {
            private_config_value(s, &dir, key, expected);
        }
        dupe_succeeds(s, &dir, &["status"]);
    });
}

/// A kill inside Git's copy of a template's `config`, which Git makes in pieces of 8 KiB
/// before `HEAD`, `objects`, and `refs`, leaves a prefix of the file alone in the private
/// directory. Run again, Git's `init` keeps the file and cannot read it: `init` fails as
/// Git fails, changing nothing, and `detach --force` then `init` start over (G21, S6).
#[test]
fn a_templates_config_left_partly_copied_is_started_over_by_detach_force_then_init() {
    under_each_release(|s| {
        let value = "x".repeat(32 * 1024);
        let config = format!("[audit]\n\tvalue = \"{value}\"\n");
        let template_dir = template(s, &[("config", config.as_bytes())]);

        let control = s.dir().join("control");
        s.repository(&control);
        let whole = init_under(s, &template_dir, &control);
        assert_eq!(whole.end, End::Code(0), "{whole:?}");
        private_config_value(s, &control, "core.worktree", "../..");
        private_config_value(s, &control, "status.showUntrackedFiles", "no");

        let killed = s.dir().join("killed");
        s.repository(&killed);
        let private = killed.join(".git/dupe");
        fs::create_dir(&private).unwrap();
        fs::write(private.join("config"), &config.as_bytes()[..8192]).unwrap();
        let before = Tree::of(&killed);
        for _ in 0..2 {
            let failed = init_under(s, &template_dir, &killed);
            assert_eq!(failed.end, End::Code(128), "{failed:?}");
            assert!(failed.stdout.is_empty(), "{failed:?}");
            // Git says so, naming the file (G21): its `fatal:` lines reach standard error,
            // their words unread, beside git-dupe's own, which here are warnings alone.
            assert!(!failed.lines("fatal").is_empty(), "{failed:?}");
            unchanged(&before, &killed);
        }
        assert!(!private.join("HEAD").exists());

        dupe_succeeds(s, &killed, &["detach", "--force"]);
        detached(&killed);
        let git = killed.join(".git");
        let after = Tree::of(&killed).without_the_lock_file(&git);
        let changed = before
            .without(&[&private])
            .without_the_lock_file(&git)
            .changed_in(&after);
        assert!(changed.is_empty(), "changed: {changed:?}");
        let started_over = init_under(s, &template_dir, &killed);
        assert_eq!(started_over.end, End::Code(0), "{started_over:?}");
        for (key, expected) in [
            ("audit.value", value.as_str()),
            ("core.worktree", "../.."),
            ("status.showUntrackedFiles", "no"),
            ("advice.statusHints", "false"),
        ] {
            private_config_value(s, &killed, key, expected);
        }
        dupe_succeeds(s, &killed, &["status"]);
    });
}

/// A file a template's `config` includes, read after the keys that `config` holds, keeps
/// its own values of them: a key whose effective value it gives is not written, so that
/// the `config`'s own values stand there, the included values are the ones Git reads, and
/// neither file is edited, by `init` or by `init` run again (F2, G1, S12).
#[test]
fn a_file_a_templates_config_includes_keeps_its_values_over_inits_keys() {
    under_each_release(|s| {
        let overrides = b"[status]\n\tshowUntrackedFiles = normal\n[advice]\n\tstatusHints = yes\n\
            [user]\n\tname = Included Name\n\temail = included@example.invalid\n";
        let config = b"[status]\n\tshowUntrackedFiles = all\n[advice]\n\tstatusHints = true\n\
            [user]\n\tname = Template Name\n\temail = template@example.invalid\n\
            [include]\n\tpath = overrides\n";
        let template_dir = template(s, &[("config", config), ("overrides", overrides)]);
        let dir = s.dir().join("workspace");
        s.repository(&dir);
        for (key, value) in [
            ("user.name", "Public Name"),
            ("user.email", "public@example.invalid"),
        ] {
            s.git(["config", key, value]).from(&dir).succeeds();
        }

        let output = init_under(s, &template_dir, &dir);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let get = |scope: &str, key: &str| {
            let output = dupe_succeeds(s, &dir, &["config", scope, "--get", key]);
            String::from_utf8(output.stdout).unwrap()
        };
        for (key, own, included) in [
            ("status.showUntrackedFiles", "all", "normal"),
            ("advice.statusHints", "true", "yes"),
            ("user.name", "Template Name", "Included Name"),
            (
                "user.email",
                "template@example.invalid",
                "included@example.invalid",
            ),
        ] {
            assert_eq!(get("--local", key), format!("{own}\n"), "{key}");
            assert_eq!(get("--includes", key), format!("{included}\n"), "{key}");
        }
        let private = dir.join(".git/dupe");
        assert_eq!(fs::read(private.join("overrides")).unwrap(), overrides);
        let configured = Tree::of(&private);

        completed_hint(&s.git(["dupe", "init"]).from(&dir).run(), false);
        unchanged(&configured, &private);
    });
}

/// What `git dupe config --includes --get <key>` answers in the workspace at `dir`.
fn effective(s: &Scenario, dir: &Path, key: &str) -> String {
    let output = dupe_succeeds(s, dir, &["config", "--includes", "--get", key]);
    String::from_utf8(output.stdout).unwrap()
}

/// What a template holding only an `[include]` gives: the four keys, each the included
/// file's.
const INCLUDED_FILE: &[u8] =
    b"[user]\n\tname = Included Name\n\temail = included@example.invalid\n\
    [status]\n\tshowUntrackedFiles = all\n[advice]\n\tstatusHints = true\n";

const INCLUDE_ALONE: &[u8] = b"[include]\n\tpath = included\n";

/// A repository at `dir` whose local configuration sets an identity of its own.
fn with_a_public_identity(s: &Scenario, dir: &Path) {
    for (key, value) in [
        ("user.name", "Public Name"),
        ("user.email", "public@example.invalid"),
    ] {
        s.git(["config", key, value]).from(dir).succeeds();
    }
}

fn the_included_values_stand(s: &Scenario, dir: &Path) {
    for (key, value) in [
        ("user.name", "Included Name\n"),
        ("user.email", "included@example.invalid\n"),
        ("status.showUntrackedFiles", "all\n"),
        ("advice.statusHints", "true\n"),
    ] {
        assert_eq!(effective(s, dir, key), value, "{key}");
    }
}

/// The identity of the next private commit, as it lands.
fn commits_as_included(s: &Scenario, dir: &Path) {
    write(dir, "notes/mine.md", b"mine\n");
    dupe_succeeds(s, dir, &["add", "notes/"]);
    dupe_succeeds(s, dir, &["commit", "-m", "mine"]);
    let log = dupe_succeeds(s, dir, &["log", "-1", "--format=%an <%ae>"]);
    assert_eq!(log.stdout, b"Included Name <included@example.invalid>\n");
}

/// Git leaves the include first and appends a key the file does not hold after it, where
/// the key would outrank the included value: a key whose effective value the include gives
/// is not written, and one it leaves unset is (F2, G1, S12).
#[test]
fn a_template_holding_only_an_include_keeps_what_the_included_file_sets() {
    under_each_release(|s| {
        let template_dir = template(s, &[("config", INCLUDE_ALONE), ("included", INCLUDED_FILE)]);
        let dir = s.dir().join("workspace");
        s.repository(&dir);
        with_a_public_identity(s, &dir);
        let output = init_under(s, &template_dir, &dir);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        the_included_values_stand(s, &dir);
        let own = s
            .private(&dir)
            .git(["config", "--local", "--no-includes", "--get", "user.name"])
            .run();
        assert_eq!(own.end, End::Code(1), "{own:?}");
        commits_as_included(s, &dir);

        // A complete workspace: nothing is written again.
        let config = fs::read(dir.join(".git/dupe/config")).unwrap();
        let again = init_under(s, &template_dir, &dir);
        assert_eq!(again.end, End::Code(0), "{again:?}");
        assert_eq!(fs::read(dir.join(".git/dupe/config")).unwrap(), config);

        // An `init` a kill left unfinished is completed under the same rule.
        s.private(&dir)
            .git(["config", "--local", "--unset", "core.worktree"])
            .succeeds();
        let again = init_under(s, &template_dir, &dir);
        assert_eq!(again.end, End::Code(0), "{again:?}");
        the_included_values_stand(s, &dir);
    });
}

#[test]
fn a_key_the_included_file_leaves_unset_is_copied_all_the_same() {
    under_each_release(|s| {
        let included = b"[user]\n\tname = Included Name\n";
        let template_dir = template(s, &[("config", INCLUDE_ALONE), ("included", included)]);
        let dir = s.dir().join("workspace");
        s.repository(&dir);
        with_a_public_identity(s, &dir);
        let output = init_under(s, &template_dir, &dir);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(effective(s, &dir, "user.name"), "Included Name\n");
        assert_eq!(effective(s, &dir, "user.email"), "public@example.invalid\n");
        assert_eq!(effective(s, &dir, "status.showUntrackedFiles"), "no\n");
        assert_eq!(effective(s, &dir, "advice.statusHints"), "false\n");
    });
}

/// A value of the file's own after the include is the effective one, and is written in
/// place, as F2 admits.
#[test]
fn a_value_of_the_config_itself_after_the_include_is_replaced_by_the_copy() {
    under_each_release(|s| {
        let config = b"[include]\n\tpath = included\n[user]\n\tname = Template Name\n";
        let included = b"[user]\n\tname = Included Name\n";
        let template_dir = template(s, &[("config", config), ("included", included)]);
        let dir = s.dir().join("workspace");
        s.repository(&dir);
        with_a_public_identity(s, &dir);
        let output = init_under(s, &template_dir, &dir);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(effective(s, &dir, "user.name"), "Public Name\n");
    });
}

#[test]
fn clone_under_a_template_holding_only_an_include_keeps_what_the_included_file_sets() {
    under_each_release(|s| {
        let template_dir = template(s, &[("config", INCLUDE_ALONE), ("included", INCLUDED_FILE)]);
        let second = s.second_machine("workspace");
        with_a_public_identity(s, &second.root);
        let words = [
            OsStr::new("-c"),
            &template_dir,
            OsStr::new("dupe"),
            OsStr::new("clone"),
            second.first.private_remote.as_os_str(),
        ];
        let output = s.git(words).from(&second.root).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        the_included_values_stand(s, &second.root);
        commits_as_included(s, &second.root);
    });
}

/// A template directory holding `files`, each name with its bytes, as the word
/// `init.templateDir=<directory>` that a `-c` before `dupe` gives.
fn template(s: &Scenario, files: &[(&str, &[u8])]) -> OsString {
    let template = s.dir().join("template");
    fs::create_dir(&template).unwrap();
    for (name, bytes) in files {
        fs::write(template.join(name), bytes).unwrap();
    }
    let mut word = OsString::from("init.templateDir=");
    word.push(&template);
    word
}

/// `git -c <template_dir> dupe init` from `dir`, as it ends.
fn init_under(s: &Scenario, template_dir: &OsStr, dir: &Path) -> Output {
    let words = [
        OsStr::new("-c"),
        template_dir,
        OsStr::new("dupe"),
        OsStr::new("init"),
    ];
    s.git(words).from(dir).run()
}
