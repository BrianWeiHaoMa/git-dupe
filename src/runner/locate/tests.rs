use super::*;
use crate::links_and_modes::{at_the_top, names_every_byte_string, remove_link, symlink};

/// What Git prints for an answer these checks spell with `/w`, `/x`, and `/elsewhere` lines:
/// paths this platform calls absolute, `C:/w` on Windows, where `/w` is not (S23).
fn printed(answer: &[u8]) -> Vec<u8> {
    let lines = answer.split_inclusive(|&byte| byte == b'\n');
    lines
        .map(|line| {
            if line.starts_with(b"/w") || line.starts_with(b"/x") {
                at_the_top(line)
            } else {
                line.to_vec()
            }
        })
        .collect::<Vec<_>>()
        .concat()
}

/// The path `/w/…` a check names, as Git would print it here.
fn at(path: &str) -> PathBuf {
    PathBuf::from(bytes::name(&printed(path.as_bytes())).expect("a name"))
}

fn read(answer: &[u8]) -> Workspace {
    Workspace::read(&printed(answer), false).expect("four answers")
}

fn read_asking_the_superproject(answer: &[u8]) -> Option<Workspace> {
    Workspace::read(&printed(answer), true)
}

fn unlocated(answer: &[u8]) -> Option<Unlocated> {
    Unlocated::read(&printed(answer))
}

#[test]
fn at_the_root_the_prefix_line_is_empty() {
    let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\n\n");
    assert_eq!(workspace.git_directory, at("/w/repo/.git"));
    assert_eq!(workspace.common_directory(), at("/w/repo/.git"));
    assert_eq!(workspace.root(), at("/w/repo"));
    assert_eq!(workspace.private_directory(), at("/w/repo/.git/dupe"));
    assert_eq!(workspace.prefix(), b"");
    assert!(!workspace.linked());
    assert!(!workspace.attached());
}

#[test]
fn below_the_root_the_answer_names_the_same_repository() {
    let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/dir/\n");
    assert_eq!(workspace.git_directory, at("/w/repo/.git"));
    assert_eq!(workspace.root(), at("/w/repo"));
    assert_eq!(workspace.prefix(), b"sub/dir/");
}

#[test]
fn a_linked_worktrees_private_repository_is_in_its_own_git_directory() {
    // An ordinary repository's, a bare one's, and one made with `--separate-git-dir`:
    // never the main worktree's, whatever the common Git directory is called.
    for (answer, private) in [
        (
            &b"/w/repo/.git/worktrees/linked\n/w/repo/.git\n/w/linked\nsub/\n"[..],
            "/w/repo/.git/worktrees/linked/dupe",
        ),
        (
            b"/w/bare.git/worktrees/one\n/w/bare.git\n/w/one\n\n",
            "/w/bare.git/worktrees/one/dupe",
        ),
        (
            b"/w/s/admin/worktrees/linked\n/w/s/admin\n/w/s/linked\n\n",
            "/w/s/admin/worktrees/linked/dupe",
        ),
    ] {
        let workspace = read(answer);
        assert!(workspace.linked(), "{}", answer.escape_ascii());
        assert_eq!(
            workspace.private_directory(),
            at(private),
            "{}",
            answer.escape_ascii()
        );
    }
}

#[test]
fn the_worktree_is_named_by_its_git_directory_alone() {
    // The main worktree has no name, its Git directory the common one.
    let main = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/\n");
    assert_eq!(main.git_directory(), at("/w/repo/.git"));
    assert_eq!(main.worktree_name(), None);
    // A linked worktree's name is its Git directory's last component, whatever its
    // root is called: an ordinary repository's and a bare one's.
    for (answer, git_directory, name) in [
        (
            &b"/w/repo/.git/worktrees/agent\n/w/repo/.git\n/w/project-agent\n\n"[..],
            "/w/repo/.git/worktrees/agent",
            &b"agent"[..],
        ),
        (
            b"/w/bare.git/worktrees/one\n/w/bare.git\n/w/elsewhere/two\nsub/\n",
            "/w/bare.git/worktrees/one",
            b"one",
        ),
    ] {
        let workspace = read(answer);
        assert_eq!(
            workspace.git_directory(),
            at(git_directory),
            "{}",
            answer.escape_ascii()
        );
        assert_eq!(
            workspace.worktree_name().map(bytes::of),
            Some(name),
            "{}",
            answer.escape_ascii()
        );
        // The private repository is the Git directory's, which is named the same.
        assert_eq!(
            workspace.private_directory(),
            at(git_directory).join("dupe")
        );
        assert!(workspace.linked());
    }
}

#[test]
fn attached_is_this_worktrees_private_directory_being_a_directory_by_lstat() {
    let scratch = env::temp_dir().join(format!("git-dupe-locate-{}", std::process::id()));
    let common = scratch.join("repo/.git");
    let linked = Workspace::linked_at(&common, b"one", &scratch.join("one"));
    let main = Workspace::at_root(&scratch.join("repo"));
    fs::create_dir_all(common.join("worktrees/one")).unwrap();
    // The main worktree attached leaves the linked one unattached.
    fs::create_dir(common.join("dupe")).unwrap();
    assert!(main.attached_now());
    assert!(!linked.attached_now());
    assert_eq!(linked.obstruction(), None);
    // A symbolic link at the linked worktree's own path is not its private repository,
    // even where it leads to one, and it obstructs that path; so does a file.
    symlink(common.join("dupe"), linked.private_directory()).unwrap();
    assert!(!linked.attached_now());
    assert_eq!(linked.obstruction(), Some(linked.private_directory()));
    remove_link(linked.private_directory()).unwrap();
    fs::write(linked.private_directory(), b"").unwrap();
    assert_eq!(linked.obstruction(), Some(linked.private_directory()));
    fs::remove_file(linked.private_directory()).unwrap();
    fs::create_dir(linked.private_directory()).unwrap();
    assert!(linked.attached_now());
    assert_eq!(linked.obstruction(), None);
    fs::remove_dir(common.join("dupe")).unwrap();
    assert!(!main.attached_now());
    assert!(linked.attached_now());
    fs::remove_dir_all(&scratch).unwrap();
}

#[test]
fn the_private_directory_is_dupe_in_the_git_directory() {
    assert_eq!(
        private_directory(&at("/w/repo/.git")),
        at("/w/repo/.git/dupe")
    );
}

/// A fresh root holding a file, a directory, a symbolic link to the directory, and a
/// dangling one, and nothing at `nothing`.
fn standing(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("git-dupe-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("directory")).unwrap();
    fs::write(root.join("file"), b"").unwrap();
    symlink("directory", root.join("link")).unwrap();
    symlink("nowhere", root.join("dangling")).unwrap();
    root
}

#[test]
fn present_is_whatever_lstat_finds_a_dangling_link_included() {
    let root = standing("present");
    for path in [&b"file"[..], b"directory", b"link", b"dangling"] {
        assert!(present(&root, path), "{}", path.escape_ascii());
    }
    for path in [&b"nothing"[..], b"file/below"] {
        assert!(!present(&root, path), "{}", path.escape_ascii());
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_directory_by_lstat_is_never_a_link_to_one() {
    let root = standing("directory-at");
    assert!(directory_at(&root, b"directory"));
    for path in [&b"file"[..], b"link", b"dangling", b"nothing"] {
        assert!(!directory_at(&root, path), "{}", path.escape_ascii());
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_nested_repository_is_a_directory_holding_a_git_entry_of_any_kind() {
    let root = standing("nested-repository-at");
    for (path, entry) in [
        ("clone", "dir"),
        ("checkout", "file"),
        ("broken", "dangling"),
    ] {
        let git = root.join(path).join(".git");
        fs::create_dir_all(root.join(path)).unwrap();
        match entry {
            "dir" => fs::create_dir(&git).unwrap(),
            "file" => fs::write(&git, b"").unwrap(),
            _ => symlink("nowhere", &git).unwrap(),
        }
        assert!(nested_repository_at(&root, path.as_bytes()), "{path}");
    }
    // A link to a nested repository is none, nor is a directory without the entry.
    symlink("clone", root.join("to-clone")).unwrap();
    for path in [&b"to-clone"[..], b"directory", b"file", b"nothing"] {
        assert!(
            !nested_repository_at(&root, path),
            "{}",
            path.escape_ascii()
        );
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn paths_are_bytes_kept_as_they_came_where_they_are_names_and_no_answer_where_they_are_not() {
    let answer = printed(b"/w/caf\xe9/.git/worktrees/x\xff\n/w/caf\xe9/.git\n/w/x\xff\n\n");
    let printed_paths = printed(b"/w/caf\xe9/.git/worktrees/x\xff/dupe");
    let workspace = Workspace::read(&answer, false);
    let locate = printed(b"/w/caf\xe9.git\n/w/caf\xe9.git\n");
    let failed = Unlocated::read(&locate);
    if !names_every_byte_string() {
        assert!(workspace.is_none());
        assert!(failed.is_none());
        return;
    }
    let workspace = workspace.expect("four answers");
    let name = |path: &[u8]| PathBuf::from(bytes::name(path).expect("a name"));
    assert_eq!(workspace.private_directory(), name(&printed_paths));
    assert_eq!(workspace.root(), name(&printed(b"/w/x\xff")));
    assert_eq!(
        workspace.worktree_name().map(bytes::of),
        Some(&b"x\xff"[..])
    );
    let Some(Unlocated::InGitDirectory { git_directory, .. }) = failed else {
        panic!("two answers");
    };
    assert_eq!(git_directory, name(&printed(b"/w/caf\xe9.git/dupe")));
}

#[test]
fn lstat_finds_nothing_at_bytes_that_are_no_name() {
    let root = standing("no-name");
    assert!(!present(&root, b"\xff"));
    assert!(!directory_at(&root, b"\xff"));
    assert!(!nested_repository_at(&root, b"\xff"));
    // A filesystem can refuse such a name though the platform has it, as APFS does.
    let named = bytes::under(&root, b"\xff/.git");
    if named.is_some_and(|path| fs::create_dir_all(path).is_ok()) {
        assert!(present(&root, b"\xff"));
        assert!(directory_at(&root, b"\xff"));
        assert!(nested_repository_at(&root, b"\xff"));
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_prefix_holding_a_newline_is_the_rest_of_the_answer() {
    let workspace = read(b"/w/repo/.git\n/w/repo/.git\n/w/repo\nnew\nline\n/sub/\n");
    assert_eq!(workspace.prefix(), b"new\nline\n/sub/");
    assert_eq!(workspace.git_directory, at("/w/repo/.git"));
    assert_eq!(workspace.common_directory, at("/w/repo/.git"));
    let workspace = read(b"/w/repo/.git/worktrees/l\n/w/repo/.git\n/w/l\nnew\nline\n/\n");
    assert_eq!(workspace.prefix(), b"new\nline\n/");
    assert_eq!(workspace.root(), at("/w/l"));
}

#[test]
fn an_answer_of_another_shape_is_not_read() {
    for answer in [
        &b""[..],
        b"/w/repo/.git\n/w/repo/.git\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/",
        b"repo/.git\n/w/repo/.git\n/w/repo\n\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub\n",
        // A newline in one of the three paths, outside the envelope, as Git prints
        // it: at the root, and below it.
        b"/w/re\npo/.git\n/w/re\npo/.git\n/w/re\npo\n\n",
        b"/w/re\n/po/.git\n/w/re\n/po/.git\n/w/re\n/po\n\n",
        b"/w/re\n/po/.git\n/w/re\n/po/.git\n/w/re\n/po\nsub/\n",
        b"/w/repo/.git/worktrees/l\n/w/repo/.git\n/w/li\nnked\n\n",
    ] {
        for superproject in [false, true] {
            assert!(
                Workspace::read(&printed(answer), superproject).is_none(),
                "{}",
                answer.escape_ascii()
            );
        }
    }
}

#[test]
fn a_failed_locate_is_read_from_what_it_printed_before_failing() {
    assert!(matches!(unlocated(b""), Some(Unlocated::Outside)));
    assert!(Unlocated::Outside.facts().is_none());
    // Inside a workspace's `.git` the two answers are the same directory; in a linked
    // worktree's Git directory the common one is the main repository's, whose private
    // repository is never the one used there: the linked worktree's own is.
    for (answer, private) in [
        (&b"/w/repo/.git\n/w/repo/.git\n"[..], "/w/repo/.git/dupe"),
        (b"/w/bare.git\n/w/bare.git\n", "/w/bare.git/dupe"),
        (
            b"/w/repo/.git/worktrees/l\n/w/repo/.git\n",
            "/w/repo/.git/worktrees/l/dupe",
        ),
    ] {
        let Some(unlocated) = unlocated(answer) else {
            panic!("{}", answer.escape_ascii());
        };
        let Unlocated::InGitDirectory {
            git_directory,
            common_directory,
        } = &unlocated
        else {
            panic!("{}", answer.escape_ascii());
        };
        assert_eq!(*git_directory, at(private), "{}", answer.escape_ascii());
        // The facts keep the common Git directory the locate printed, and its parent
        // stands for the root.
        let facts = unlocated.facts().expect("facts inside a Git directory");
        assert_eq!(facts.common_directory(), common_directory);
        assert_eq!(Some(facts.root()), common_directory.parent());
        assert!(facts.outside().is_some());
    }
    for answer in [
        &b"/w/repo/.git\n"[..],
        b"/w/repo/.git\n/w/repo/.git",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\n",
        b".git\n.git\n",
        b"\n\n",
    ] {
        assert!(unlocated(answer).is_none(), "{}", answer.escape_ascii());
    }
}

#[test]
fn the_superproject_is_asked_immediately_before_the_prefix() {
    assert_eq!(words(false), LOCATE);
    assert_eq!(
        words(true),
        [
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
            "--show-superproject-working-tree",
            "--show-prefix",
        ]
    );
}

#[test]
fn asked_for_the_superproject_outside_a_submodule_the_answer_keeps_four_lines() {
    for answer in [
        &b"/w/repo/.git\n/w/repo/.git\n/w/repo\n\n"[..],
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nsub/dir/\n",
        b"/w/repo/.git\n/w/repo/.git\n/w/repo\nnew\nline\n/\n",
    ] {
        let workspace = read_asking_the_superproject(answer).expect("four answers");
        assert!(!workspace.submodule_checkout(), "{}", answer.escape_ascii());
        assert_eq!(workspace.root(), at("/w/repo"));
    }
}

#[test]
fn a_superproject_line_before_the_prefix_marks_a_submodule_checkout() {
    for (answer, root) in [
        (
            &b"/w/super/.git/modules/sub\n/w/super/.git/modules/sub\n/w/super/sub\n/w/super\n\n"[..],
            "/w/super/sub",
        ),
        (
            b"/w/s/.git/modules/m\n/w/s/.git/modules/m\n/w/s/m\n/w/s\nnew\nline\n/x/\n",
            "/w/s/m",
        ),
    ] {
        let workspace = read_asking_the_superproject(answer).expect("five answers");
        assert!(workspace.submodule_checkout(), "{}", answer.escape_ascii());
        assert_eq!(workspace.root(), at(root));
        assert!(!workspace.prefix().starts_with(b"/"));
    }
    // Not asked, the same answer is of another shape; and a superproject line must
    // be followed by a prefix.
    let submodule =
        b"/w/super/.git/modules/sub\n/w/super/.git/modules/sub\n/w/super/sub\n/w/super\n\n";
    assert!(Workspace::read(&printed(submodule), false).is_none());
    let asked = read_asking_the_superproject;
    assert!(asked(b"/w/r/.git\n/w/r/.git\n/w/r\n/w/super\n").is_none());
    assert!(asked(b"/w/r/.git\n/w/r/.git\n/w/r\n/w/s\n/x/\n").is_none());
}
