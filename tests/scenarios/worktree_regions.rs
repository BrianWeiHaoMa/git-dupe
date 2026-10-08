//! Other worktrees' regions in `.git/info/exclude`, as another worktree's command would
//! leave them, around the main worktree's: every command changes the main worktree's
//! region alone and copies every other byte through in its order (F3, G3, G6, G9).

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::Path;

use crate::harness::{
    End, Region, Tree, holds, names, refused_stash_untracked, region, region_in, run_traced,
    unchanged, under_each_release, write,
};

/// The user's text before every region.
const TOP: &[u8] = b"# the user's own\n*.o\n";
/// The region of the linked worktree `agent`, one of whose paths, `shared`, stands in the
/// main worktree too.
const AGENT: &[u8] =
    b"# BEGIN git-dupe worktree agent\n/agent-notes\n/shared\n# END git-dupe worktree agent\n";
/// The user's text between the regions.
const BETWEEN: &[u8] = b"between\n";
/// The region of a linked worktree whose name is not UTF-8.
const CAFE: &[u8] =
    b"# BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n";
/// The user's last line, without a newline.
const LAST: &[u8] = b"# last, without a newline";

/// The main worktree's region holding `rules`.
fn main_region(rules: &[&[u8]]) -> Vec<u8> {
    let mut region = b"# BEGIN git-dupe\n".to_vec();
    for rule in rules {
        region.extend_from_slice(rule);
        region.push(b'\n');
    }
    region.extend_from_slice(b"# END git-dupe\n");
    region
}

/// The whole file with the main worktree's region `main` between the others.
fn around(main: &[u8]) -> Vec<u8> {
    [TOP, AGENT, BETWEEN, main, CAFE, LAST].concat()
}

/// The device and inode at `path`: the same pair after a command is the same file.
fn identity(path: &Path) -> (u64, u64) {
    let found = fs::symlink_metadata(path).unwrap();
    (found.dev(), found.ino())
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

#[test]
fn every_command_changes_the_main_region_alone_and_copies_the_rest_in_order() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        s.attached_repository(&root);
        let exclude = root.join(".git/info/exclude");
        fs::write(&exclude, around(&main_region(&[b"/.gitdupe"]))).unwrap();
        fs::set_permissions(&exclude, fs::Permissions::from_mode(0o640)).unwrap();
        // `shared` stands here and only the other worktree's region names it; a rule of
        // the user's makes public Git see it here, so that a false release would be named.
        write(&root, "shared", b"here\n");
        write(&root, ".gitignore", b"!/shared\n");

        let hide = s.git(["dupe", "hide", "notes"]).from(&root).succeeds();
        assert!(hide.lines("warning").is_empty(), "{hide:?}");
        assert_eq!(
            fs::read(&exclude).unwrap(),
            around(&main_region(&[b"/.gitdupe", b"/notes"]))
        );
        assert_eq!(mode(&exclude), 0o640);
        // The named reader finds each region by its own markers, from the common Git
        // directory.
        let common = root.join(".git");
        assert_eq!(
            region_in(&common, Some(OsStr::new("agent"))),
            Some(Region {
                before: TOP.to_vec(),
                rules: vec![b"/agent-notes".to_vec(), b"/shared".to_vec()],
                after: [
                    BETWEEN,
                    &main_region(&[b"/.gitdupe", b"/notes"]),
                    CAFE,
                    LAST
                ]
                .concat(),
            })
        );
        assert_eq!(
            region_in(&common, Some(OsStr::from_bytes(b"caf\xe9"))).map(|found| found.rules),
            Some(vec![b"/caf\xe9".to_vec()])
        );
        assert_eq!(
            region(&root).map(|found| found.rules),
            Some(vec![b"/.gitdupe".to_vec(), b"/notes".to_vec()])
        );

        // A file made below the hidden directory is ignored by public Git with no further
        // command, and so is `.gitdupe` (F5, G7).
        write(&root, "notes/new.md", b"new\n");
        let public = s
            .git(["status", "--porcelain", "--untracked-files=all"])
            .from(&root)
            .succeeds();
        assert!(!holds(&public.stdout, b"notes"), "{public:?}");
        assert!(!holds(&public.stdout, b".gitdupe"), "{public:?}");
        assert!(holds(&public.stdout, b"shared"), "{public:?}");

        write(&root, "docs/private.md", b"private\n");
        let add = s
            .git(["dupe", "add", "docs/private.md"])
            .from(&root)
            .succeeds();
        assert!(add.lines("warning").is_empty(), "{add:?}");
        let three = around(&main_region(&[
            b"/.gitdupe",
            b"/docs/private.md",
            b"/notes",
        ]));
        assert_eq!(fs::read(&exclude).unwrap(), three);

        // A refusal settles, and a composition equal to the file writes nothing.
        let written = identity(&exclude);
        let refused = s.git(["dupe", "stash", "-u"]).from(&root).run();
        refused_stash_untracked(s, &refused);
        assert!(refused.lines("warning").is_empty(), "{refused:?}");
        assert_eq!(fs::read(&exclude).unwrap(), three);
        assert_eq!(identity(&exclude), written);

        // The released directory is named; the path only the other region names is not.
        let unhide = s.git(["dupe", "unhide", "notes"]).from(&root).succeeds();
        let warnings = unhide.lines("warning");
        assert_eq!(warnings.len(), 1, "{unhide:?}");
        names(warnings[0], b"notes");
        assert_eq!(
            fs::read(&exclude).unwrap(),
            around(&main_region(&[b"/.gitdupe", b"/docs/private.md"]))
        );
        assert_eq!(mode(&exclude), 0o640);

        // `detach` takes the main region and the private repository, nothing else.
        let detach = s.git(["dupe", "detach", "--force"]).from(&root).succeeds();
        assert!(
            !detach
                .lines("warning")
                .iter()
                .any(|line| holds(line, b"shared") || holds(line, b"agent")),
            "{detach:?}"
        );
        let without = [TOP, AGENT, BETWEEN, CAFE, LAST].concat();
        assert_eq!(fs::read(&exclude).unwrap(), without);
        assert!(!root.join(".git/dupe").exists());
        for path in ["notes/new.md", "docs/private.md", "shared", ".gitdupe"] {
            assert!(root.join(path).exists(), "{path}");
        }

        // Attached again, its region is appended after everything else.
        s.init(&root);
        let appended = [&without[..], b"\n", &main_region(&[b"/.gitdupe"])].concat();
        assert_eq!(fs::read(&exclude).unwrap(), appended);
        let written = identity(&exclude);
        let status = s.git(["dupe", "status"]).from(&root).succeeds();
        assert!(status.lines("warning").is_empty(), "{status:?}");
        assert_eq!(fs::read(&exclude).unwrap(), appended);
        assert_eq!(identity(&exclude), written);
    });
}

#[test]
fn detach_refuses_a_link_at_info_only_while_the_main_region_stands_beyond_it() {
    under_each_release(|s| {
        for main_beyond in [false, true] {
            let root = s.dir().join(format!("beyond-{main_beyond}"));
            s.unattached_project(&root);
            let info = root.join(".git/info");
            let target = s.dir().join(format!("info-elsewhere-{main_beyond}"));
            fs::rename(&info, &target).unwrap();
            let main: &[u8] = if main_beyond {
                &main_region(&[b"/.gitdupe"])
            } else {
                b""
            };
            fs::write(target.join("exclude"), around(main)).unwrap();
            symlink(&target, &info).unwrap();
            s.init(&root);
            let link = fs::read_link(&info).unwrap();
            let beyond = Tree::of(&target);

            if main_beyond {
                for words in [
                    ["dupe", "detach"].as_slice(),
                    &["dupe", "detach", "--force"],
                ] {
                    let (output, runs) =
                        run_traced(s.git(words).from(&root), &s.dir().join("trace"));
                    assert_eq!(output.end, End::Code(128), "{output:?}");
                    names(output.only_line("fatal"), info.as_os_str().as_bytes());
                    let own = runs.own();
                    assert_eq!(own.commands(), [b"rev-parse".as_slice()], "{own:?}");
                    assert!(root.join(".git/dupe").is_dir());
                }
            } else {
                // Another worktree's region there is not the main worktree's: `detach`
                // proceeds and leaves the link and everything beyond it.
                s.git(["dupe", "detach"]).from(&root).succeeds();
                assert!(!root.join(".git/dupe").exists());
            }
            assert_eq!(fs::read_link(&info).unwrap(), link);
            unchanged(&beyond, &target);
            assert_eq!(fs::read(target.join("exclude")).unwrap(), around(main));
        }
    });
}
