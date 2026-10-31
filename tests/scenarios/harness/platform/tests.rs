use std::fs;

use super::super::tree::Tree;
use super::*;

/// A valid name is one everywhere; bytes that are no UTF-8 are one exactly off Windows.
#[test]
fn a_name_is_valid_utf8_on_windows_and_any_bytes_elsewhere() {
    assert_eq!(
        name(b"caf\xc3\xa9").unwrap().into_string().as_deref(),
        Ok("café")
    );
    assert_eq!(name(b"\xff").is_some(), word() != "windows");
}

/// The scenarios' directory reads back through its spelling as Git's, and a canonical path
/// is a word Git accepts.
#[test]
fn a_canonical_path_is_spelled_as_git_spells_it() {
    let fresh = FreshDirectory::create();
    let path = canonical(fresh.path()).unwrap();
    assert!(!path.to_str().unwrap().starts_with(r"\\?\"), "{path:?}");
    assert_eq!(path, fresh.path());
    let spelled = as_git_spells(&path);
    assert!(!spelled.contains('\\'), "{spelled}");
    assert_eq!(Path::new(&spelled), path);
}

/// The byte spelling agrees with the text's, on a path of the scenarios' directory.
#[test]
fn a_path_is_spelled_alike_as_text_and_as_bytes() {
    let fresh = FreshDirectory::create();
    let path = fresh.path().join("sub").join("file");
    assert_eq!(
        as_git_spells_bytes(&path),
        as_git_spells(&path).into_bytes()
    );
}

/// Two files differ, a write in place keeps a file's identity, and a rename onto its path
/// changes it.
#[test]
fn an_identity_follows_the_file_and_not_its_path() {
    let fresh = FreshDirectory::create();
    let [a, b, c] = ["a", "b", "c"].map(|name| fresh.path().join(name));
    for file in [&a, &b, &c] {
        fs::write(file, b"first\n").unwrap();
    }
    let before = identity(&a);
    assert_ne!(before, identity(&b));
    fs::write(&a, b"second\n").unwrap();
    assert_eq!(identity(&a), before);
    fs::rename(&c, &a).unwrap();
    assert_ne!(identity(&a), before);
}

/// A link is an entry of its own, as `lstat` sees it, and making one leaves its target's
/// identity as it was.
#[test]
fn a_links_identity_is_its_own() {
    let fresh = FreshDirectory::create();
    let (target, link) = (fresh.path().join("target"), fresh.path().join("link"));
    fs::write(&target, b"data\n").unwrap();
    let before = identity(&target);
    symlink(&target, &link).unwrap();
    assert_ne!(identity(&link), identity(&target));
    assert_eq!(identity(&target), before);
    remove_link(&link).unwrap();
    assert!(fs::symlink_metadata(&link).is_err());
    assert!(target.exists());
}

/// The limit a check of a kernel limit takes is Windows's number there alone.
#[test]
fn the_command_line_limit_is_windows_s_there() {
    assert_eq!(command_line_limit() == 32_767, word() == "windows");
}

/// Mode bits are read and made exactly where they exist.
#[test]
fn modes_exist_together() {
    let fresh = FreshDirectory::create();
    let file = fresh.path().join("file");
    fs::write(&file, b"data\n").unwrap();
    assert_eq!(permissions(0o640).is_some(), mode(&file).is_some());
    if let Some(bits) = permissions(0o640) {
        fs::set_permissions(&file, bits).unwrap();
        assert_eq!(mode(&file), Some(0o640));
    }
    assert_eq!(mode(&file).is_some(), word() != "windows");
}

/// Signals and the file size limit stand or fall with the platform.
#[test]
fn what_windows_lacks_is_answered_as_lacking() {
    assert_eq!(signals(), word() != "windows");
    assert_eq!(file_size_limits(), word() != "windows");
}

/// A name no platform can make refuses nothing valid, and a valid name leaves nothing.
#[test]
fn a_valid_name_is_accepted_and_leaves_nothing() {
    let fresh = FreshDirectory::create();
    name_accepted(fresh.path(), b"caf\xc3\xa9").unwrap();
    assert_eq!(fs::read_dir(fresh.path()).unwrap().count(), 0);
}

/// The release's programs and shell lie below the release, but for the machine's own
/// `/bin/sh`, and its `git` in the first of the directories that lead `PATH`.
#[test]
fn the_release_is_reached_through_its_own_layout() {
    let release = Path::new("gits").join("git-2.48.0");
    let programs = release_programs(&release);
    assert!(programs.iter().all(|program| program.starts_with(&release)));
    assert_eq!(
        release_git(&release).parent(),
        programs.first().map(|p| &**p)
    );
    assert_eq!(
        shell(&release).starts_with(&release),
        word() == "windows",
        "{}",
        shell(&release).display()
    );
    let build = match word() {
        "windows" => "2.48.1.windows.1",
        _ => "2.48.0",
    };
    assert_eq!(git_version_line("2.48.0"), format!("git version {build}\n"));
}

/// Windows's environment passes four variables through and sets the one that makes links;
/// elsewhere a scenario's environment needs none.
#[test]
fn the_environment_needs_nothing_but_on_windows() {
    let windows = word() == "windows";
    assert_eq!(!environment_passed_through().is_empty(), windows);
    assert_eq!(
        environment_set().contains(&("MSYS", "winsymlinks:nativestrict")),
        windows
    );
    assert_eq!(environment_set().len(), usize::from(windows));
}

/// Every platform has a way, and each way answers.
#[test]
fn every_way_to_a_second_filesystem_is_named() {
    let ways = second_filesystems();
    assert!(!ways.is_empty());
    for way in ways {
        assert!(!way.reached.is_empty());
        if let Ok(second) = way.found {
            assert_ne!(identity(second.path()).0, identity(&std::env::temp_dir()).0);
            assert!(Tree::of(second.path()).paths().next().is_none());
        }
    }
}
