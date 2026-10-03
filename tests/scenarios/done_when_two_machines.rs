//! The product's `Done when`, its fourth bullet as one story on two clones of one project,
//! in the order written: the second machine attaches and keeps its own `.env.local`,
//! private commits move both ways, seven commands are each refused changing nothing on
//! either machine, two words a table does not hold run nothing, `git dupe git` is Git's
//! own, and `detach --force` leaves the files. Each clause has a scenario of its own as
//! well; here each command meets the state the one before it left. Every `git dupe`
//! command of the story leaves the public `.git` of its clone as it was but for
//! `.git/dupe` and the exclude file, and when the story ends neither public repository
//! holds anything private.

use std::fs;
use std::path::Path;

use crate::harness::{
    EVERY_OBJECT, End, Output, PROJECT_URL, Runs, Scenario, SecondMachine, Tree, changed_since,
    detached, held_publicly, holds, leaving_public_git, names, now_visible, privately_tracked,
    public_git, publicly_tracked, records, region_rules, run_traced, stale_timestamp,
    stash_untracked_line, under_each_release, usage_line, warnings, write,
};

/// The privately tracked files of the first machine's history other than `.env.local`:
/// each arrives on the second machine as committed.
const ARRIVING: [&str; 4] = [
    ".gitdupe",
    "notes/a.md",
    ".vscode/settings.json",
    "docs/notes.md",
];

/// `git dupe <words>` from `from` in the workspace at `root`, which must leave the public
/// `.git` there as it was but for `.git/dupe` and the exclude file.
fn dupe(s: &Scenario, root: &Path, from: &Path, words: &[&str]) -> Output {
    leaving_public_git(root, s.git(["dupe"].iter().chain(words)).from(from))
}

/// `dupe` from the root, which must exit 0.
fn dupe_succeeds(s: &Scenario, root: &Path, words: &[&str]) -> Output {
    let output = dupe(s, root, root, words);
    assert_eq!(output.end, End::Code(0), "git dupe {words:?}: {output:?}");
    output
}

/// The commit the private `HEAD` of the workspace at `root` names.
fn private_head(s: &Scenario, root: &Path) -> Vec<u8> {
    s.private(root).git(["rev-parse", "HEAD"]).succeeds().stdout
}

/// Runs `git dupe <words>` from `from` on the second machine, traced, and returns its one
/// `fatal:` line and the Git runs it made, once it has exited 128 with nothing on standard
/// output and left every byte of both machines as it was just before: both roots, their
/// `.git` and `.git/dupe` included, both linked worktrees, and the bare repositories. Both
/// workspaces are settled when they are read, so that the refusal's own settle (G6) has
/// nothing to change either.
fn refused(
    s: &Scenario,
    m: &SecondMachine,
    linked: &Path,
    from: &Path,
    words: &[&str],
) -> (Vec<u8>, Runs) {
    let before = m.everything(&[linked]);
    let git = s.git(["dupe"].iter().chain(words)).from(from);
    let (output, runs) = run_traced(git, &s.dir().join("trace"));
    assert_eq!(output.end, End::Code(128), "git dupe {words:?}: {output:?}");
    assert!(output.stdout.is_empty(), "git dupe {words:?}: {output:?}");
    let line = output.only_line("fatal").to_vec();
    before.unchanged();
    (line, runs)
}

/// A transfer refused for naming a public place (G18): the line names the place and
/// `git dupe git`, Git never ran the refused command, and settle asked about exposure.
fn public_place_refused(line: &[u8], runs: &Runs, command: &str, place: &str) {
    names(line, place.as_bytes());
    names(line, b"git dupe git");
    let own = runs.own();
    assert_eq!(own.of(command), 0, "{own:?}");
    assert_eq!(own.of("check-ignore"), 1, "{own:?}");
}

#[test]
fn two_machines_share_private_work_refusals_change_nothing_and_detach_keeps_every_file() {
    under_each_release(|s| {
        // The developer's identity for the private commits of both machines, and no
        // automatic maintenance, which a commit would leave running in the background,
        // writing below `.git/dupe` while a refusal's comparison reads it.
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        // The second clone stands for one made from the project's host: its public
        // `origin` names the project's URL, which nothing contacts. Beside it, a linked
        // worktree of its public repository; in its exclude file, a rule of its own.
        s.git(["remote", "set-url", "origin", PROJECT_URL])
            .from(second)
            .succeeds();
        let linked = s.dir().join("project-second-linked");
        s.linked_worktree(second, &linked);
        let exclude = second.join(".git/info/exclude");
        let mut own_exclude = fs::read(&exclude).unwrap();
        own_exclude.extend_from_slice(b"*.orig\n");
        fs::write(&exclude, &own_exclude).unwrap();

        // "On a second clone of the project holding its own differing `.env.local`,
        // `git dupe clone URL` leaves that file unchanged, warns, shows it modified, and
        // brings `.gitdupe`, so that a new `notes/x.md` there is invisible to
        // `git status`;"
        write(second, ".env.local", b"mine\n");
        let url = m.first.private_remote.to_str().unwrap();
        let cloned = dupe_succeeds(s, second, &["clone", url]);
        for level in ["fatal", "error", "hint"] {
            assert!(cloned.lines(level).is_empty(), "{level}: {cloned:?}");
        }
        let warned = cloned.lines("warning");
        assert_eq!(warned.len(), 1, "{cloned:?}");
        names(warned[0], b".env.local");
        for path in ARRIVING {
            assert!(!holds(warned[0], path.as_bytes()), "{path}: {cloned:?}");
        }
        assert_eq!(fs::read(second.join(".env.local")).unwrap(), b"mine\n");
        for path in ARRIVING {
            assert_eq!(
                fs::read(second.join(path)).unwrap(),
                fs::read(first.join(path)).unwrap(),
                "{path}"
            );
        }
        // The region `clone` left is the first machine's: before any other `git dupe`
        // command runs, a new file under a hidden directory is invisible to public Git.
        assert_eq!(region_rules(second), region_rules(first));
        write(second, "notes/x.md", b"new\n");
        let public = s
            .git(["status", "--porcelain", "--untracked-files=all"])
            .from(second)
            .succeeds();
        assert!(public.stdout.is_empty(), "{public:?}");
        let status = dupe_succeeds(s, second, &["status", "--porcelain"]);
        assert_eq!(
            status.stdout, b" M .env.local\n?? notes/x.md\n",
            "{status:?}"
        );

        // "`git dupe pull` and `git dupe push` move commits both ways." The second machine
        // keeps its `.env.local` with `git dupe commit -am` (`In use` 5) and sends it to
        // the first; the first sends a new note back.
        dupe_succeeds(
            s,
            second,
            &["commit", "-qam", "the second machine's settings"],
        );
        dupe_succeeds(s, second, &["push"]);
        dupe_succeeds(s, first, &["pull"]);
        assert_eq!(private_head(s, first), private_head(s, second));
        assert_eq!(fs::read(first.join(".env.local")).unwrap(), b"mine\n");
        write(first, "notes/b.md", b"from the first machine\n");
        for words in [
            &["add", "notes/b.md"][..],
            &["commit", "-qm", "a note"],
            &["push"],
        ] {
            dupe_succeeds(s, first, words);
        }
        dupe_succeeds(s, second, &["pull"]);
        assert_eq!(private_head(s, second), private_head(s, first));
        assert_eq!(
            fs::read(second.join("notes/b.md")).unwrap(),
            b"from the first machine\n"
        );

        // Then the second machine leaves its private remote with
        // `git dupe remote remove origin`: no remote holds its commits for `detach`, and a
        // `remote add origin` that no guard stopped would succeed. Nothing is uncommitted;
        // `notes/x.md` stays untracked, for `stash -u` to take.
        dupe_succeeds(s, second, &["remote", "remove", "origin"]);

        // "`git dupe remote add origin <the project's remote URL>`, … are each refused,
        // exit 128, naming the alternative, and change neither index, the working tree,
        // `.gitdupe`, the private configuration, the public `.git` beyond its managed
        // region, nor any ref;"
        let (line, runs) = refused(
            s,
            &m,
            &linked,
            second,
            &["remote", "add", "origin", PROJECT_URL],
        );
        public_place_refused(&line, &runs, "remote", "origin");
        // "`git dupe push <that URL in another scheme> main:leak`"
        let another_scheme = "ssh://git@example.com/team/project";
        let (line, runs) = refused(
            s,
            &m,
            &linked,
            second,
            &["push", another_scheme, "main:leak"],
        );
        public_place_refused(&line, &runs, "push", "origin");
        // "`git dupe push . main:leak` from a subdirectory"
        let (line, runs) = refused(
            s,
            &m,
            &linked,
            &second.join("docs"),
            &["push", ".", "main:leak"],
        );
        let root = second.to_str().unwrap();
        public_place_refused(&line, &runs, "push", root);
        // "`git dupe fetch <the root's absolute path>`"
        let (line, runs) = refused(s, &m, &linked, second, &["fetch", root]);
        public_place_refused(&line, &runs, "fetch", root);
        // "`git dupe push <a linked worktree's path> main:leak`"
        let linked_path = linked.to_str().unwrap();
        let (line, runs) = refused(s, &m, &linked, second, &["push", linked_path, "main:leak"]);
        public_place_refused(&line, &runs, "push", linked_path);
        // "`git dupe stash -u`": `notes/x.md` is there to take.
        let (line, runs) = refused(s, &m, &linked, second, &["stash", "-u"]);
        assert_eq!(line, stash_untracked_line(s));
        let own = runs.own();
        assert_eq!(own.of("stash"), 0, "{own:?}");
        assert_eq!(own.of("check-ignore"), 1, "{own:?}");
        // "and `git dupe detach` without a remote": a privately tracked file's timestamp
        // is stale, so that a status run that could take the private index's lock would
        // rewrite the index (`Holds/G3`).
        stale_timestamp(second);
        let (line, _) = refused(s, &m, &linked, second, &["detach"]);
        names(&line, b"git dupe detach --force");
        names(&line, b"no remote is configured");

        // "`git dupe add --pathspec-from-file=x` and `git dupe status --porc` exit 129
        // naming `git dupe git`, running nothing,"
        for (command, word) in [("add", "--pathspec-from-file=x"), ("status", "--porc")] {
            let text = s.git(["dupe", "help", command]).succeeds().stdout;
            let before = m.everything(&[&linked]);
            let git = s.git(["dupe", command, word]).from(second);
            let (output, runs) = run_traced(git, &s.dir().join("trace"));
            assert_eq!(output.end, End::Code(129), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let line = output.line_then("error", usage_line(&text));
            names(line, word.as_bytes());
            names(line, b"git dupe git");
            assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
            before.unchanged();
        }

        // "and `git dupe git status --porc` is Git's own reading of the abbreviation":
        // what the release's own `git status --porc` prints in the private repository. The
        // developer edits `.env.local` again first, so that Git has a change to print.
        write(second, ".env.local", b"mine, edited\n");
        let expected = s
            .private(second)
            .git(["-c", "help.autocorrect=0", "status", "--porc"])
            .run();
        assert!(!expected.stdout.is_empty(), "{expected:?}");
        let before = public_git(second);
        let git = s.git(["dupe", "git", "status", "--porc"]).from(second);
        let (output, runs) = run_traced(git, &s.dir().join("trace"));
        assert_eq!(output, expected);
        let changed = before.changed_in(&public_git(second));
        assert!(
            changed.is_empty(),
            "the public repository changed: {changed:?}"
        );
        // Git was given the words as typed, and the command then settled (G6).
        let own = runs.own();
        let statuses: Vec<&[Vec<u8>]> = own
            .words()
            .iter()
            .zip(own.commands())
            .filter(|(_, command)| *command == b"status")
            .map(|(words, _)| words.as_slice())
            .collect();
        assert_eq!(statuses.len(), 1, "{own:?}");
        assert!(
            statuses[0].ends_with(&[b"status".to_vec(), b"--porc".to_vec()]),
            "{own:?}"
        );
        assert_eq!(own.of("check-ignore"), 1, "{own:?}");

        // Gathered before the second machine's private repository goes: the objects of
        // the private history and the privately tracked paths, on both machines.
        let mut private_objects = Vec::new();
        let mut private_paths = Vec::new();
        for root in [first, second] {
            private_objects.extend(s.private(root).git(EVERY_OBJECT).succeeds().stdout);
            private_paths.extend(privately_tracked(s, root));
        }

        // "`git dupe detach --force` leaves the files on disk, removes `.git/dupe` and the
        // region, and makes `git status` list `notes/` and `.gitdupe`."
        let working = Tree::working(second);
        let left = dupe_succeeds(s, second, &["detach", "--force"]);
        assert!(left.stdout.is_empty(), "{left:?}");
        let exposed = [
            ".gitdupe",
            "notes",
            "notes/a.md",
            "notes/b.md",
            "docs/notes.md",
        ];
        warnings(&left, exposed.map(now_visible).to_vec());
        assert!(changed_since(&working, second).is_empty());
        detached(second);
        assert_eq!(fs::read(&exclude).unwrap(), own_exclude);
        let public = s
            .git(["status", "--porcelain", "-z"])
            .from(second)
            .succeeds();
        let mut listed = records(&public.stdout, 0);
        listed.sort();
        assert_eq!(
            listed,
            [b"?? .gitdupe".as_slice(), b"?? docs/notes.md", b"?? notes/"],
            "{public:?}"
        );

        // When the story ends: neither public repository holds an object of the private
        // history or tracks a path the private repositories tracked.
        for root in [first, second] {
            assert_eq!(
                held_publicly(s, root, &private_objects),
                Vec::<Vec<u8>>::new()
            );
            let public_paths = publicly_tracked(s, root);
            assert!(!public_paths.is_empty());
            assert!(
                !public_paths.iter().any(|path| private_paths.contains(path)),
                "public: {public_paths:?}; private: {private_paths:?}"
            );
        }
    });
}
