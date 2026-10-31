use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::super::SecondFilesystem;
use super::*;

/// A process's end is its signaled state: one that exited with `STILL_ACTIVE`'s value has
/// ended once its `Child` has been waited for, its handle closed.
#[test]
fn a_process_that_exited_with_still_actives_value_has_ended() {
    let mut child = Command::new("cmd")
        .args(["/c", "exit 259"])
        .spawn()
        .unwrap();
    let process = child.id().to_string();
    assert_eq!(child.wait().unwrap().code(), Some(259));
    drop(child);
    assert!(ended(&process));
}

/// What a command starts stays in the job after the command exits: the child stands in
/// `running` with its name until `kill` ends it, and `ended` then turns true.
#[test]
fn a_job_names_its_child_until_it_is_killed() {
    let mut command = Command::new("cmd");
    command
        .args(["/c", "start /b ping -n 120 127.0.0.1 >nul"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let (mut child, group) = spawn_grouped(&mut command).unwrap();
    child.wait().unwrap();
    let exited = format!("{} ", child.id());
    let deadline = Instant::now() + Duration::from_secs(20);
    let row = loop {
        let rows = group.running().unwrap();
        if let Some(row) = rows.iter().find(|row| row.contains("PING")) {
            break row.clone();
        }
        assert!(Instant::now() < deadline, "no child in the job: {rows:?}");
    };
    let process = row.split_whitespace().next().unwrap().to_owned();
    assert!(!ended(&process), "{row}");
    // Its parent has exited, and this process still holds a handle on it.
    let rows = group.running().unwrap();
    assert!(!rows.iter().any(|row| row.starts_with(&exited)), "{rows:?}");
    group.kill();
    while !group.running().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the child still runs");
    }
    assert!(ended(&process));
}

/// Each name Windows cannot hold is refused before anything is made, and a name it holds
/// leaves nothing behind.
#[test]
fn the_names_windows_cannot_hold_are_refused_leaving_nothing() {
    let fresh = FreshDirectory::create();
    let dir = fresh.path();
    for name in [
        &b"a:b"[..],
        b"trail.",
        b"trail ",
        b"NUL",
        b"nul.txt",
        b"COM1",
        b"a\xffb",
        b"star*",
        b"back\\slash",
    ] {
        let reason = name_accepted(dir, name).unwrap_err();
        assert!(reason.contains("a name Windows cannot hold"), "{reason}");
        assert_eq!(fs::read_dir(dir).unwrap().count(), 0, "{reason}");
    }
    for name in [&b"caf\xc3\xa9"[..], b"COM10", b"nullable"] {
        name_accepted(dir, name).unwrap();
        assert_eq!(fs::read_dir(dir).unwrap().count(), 0);
    }
}

/// The verbatim prefix goes, for a drive and for a share.
#[test]
fn a_canonical_path_holds_no_verbatim_prefix() {
    let found = canonical(&std::env::temp_dir()).unwrap();
    assert!(!found.to_str().unwrap().starts_with(r"\\?\"), "{found:?}");
}

/// The letter's volume is not the first drive's.
#[test]
fn a_second_volume_is_another_volume() {
    for way in second_filesystems() {
        let Ok(second): Result<SecondFilesystem, _> = way.found else {
            continue;
        };
        assert_ne!(
            identity(second.path()).0,
            identity(&std::env::temp_dir()).0,
            "{}",
            way.reached
        );
    }
}
