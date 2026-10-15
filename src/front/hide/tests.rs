use super::*;

fn words(words: &[&[u8]]) -> Vec<OsString> {
    words
        .iter()
        .map(|word| OsStr::from_bytes(word).to_owned())
        .collect()
}

fn paths<'w>(words: &[&'w [u8]]) -> Asked<'w> {
    Asked::Paths(words.iter().map(|word| OsStr::from_bytes(word)).collect())
}

#[test]
fn every_word_is_a_path_and_after_the_end_of_options_a_dashed_one_too() {
    for (given, expected) in [
        (&[&b"notes/"[..]][..], &[&b"notes/"[..]][..]),
        (&[b"a", b"b", b"a"], &[b"a", b"b", b"a"]),
        (&[b"--", b"-x"], &[b"-x"]),
        // The first `--` ends the options; a later one is a path.
        (&[b"a", b"--", b"-h", b"--"], &[b"a", b"-h", b"--"]),
        (
            &[b"caf\xe9", b"a b\r", b"x!#\\"],
            &[b"caf\xe9", b"a b\r", b"x!#\\"],
        ),
        (
            &[b"", b"../x", b"/abs", b"a:b"],
            &[b"", b"../x", b"/abs", b"a:b"],
        ),
    ] {
        assert_eq!(read(&words(given)), paths(expected), "{given:?}");
    }
}

#[test]
fn a_help_request_anywhere_before_the_end_of_options_decides() {
    for request in [
        &[&b"-h"[..]][..],
        &[b"--help"],
        &[b"x", b"-h"],
        &[b"-f", b"--help"],
        &[b"a*", b"-h"],
        &[b"x", b"-h", b"--", b"y"],
    ] {
        assert_eq!(read(&words(request)), Asked::Help, "{request:?}");
    }
}

#[test]
fn an_option_no_path_and_a_word_that_is_no_literal_path_are_misuses_naming_it() {
    let word = |word: &'static [u8]| OsStr::from_bytes(word);
    for (misuse, fault) in [
        (&[][..], Fault::NoPath),
        (&[&b"--"[..]], Fault::NoPath),
        (&[b"-f", b"x"], Fault::Option(word(b"-f"))),
        (&[b"x", b"--force"], Fault::Option(word(b"--force"))),
        (&[b"-"], Fault::Option(word(b"-"))),
        (&[b"a*"], Fault::Pattern(word(b"a*"))),
        (&[b"a?"], Fault::Pattern(word(b"a?"))),
        (&[b"a[1]"], Fault::Pattern(word(b"a[1]"))),
        (&[b"--", b"x*"], Fault::Pattern(word(b"x*"))),
        (&[b":/x"], Fault::Magic(word(b":/x"))),
        (&[b":x"], Fault::Magic(word(b":x"))),
        (&[b"x", b"--", b":"], Fault::Magic(word(b":"))),
        // A newline is found before anything that would name the word.
        (&[b"a\nb"], Fault::Newline),
        (&[b"-\n"], Fault::Newline),
        (&[b"*\n"], Fault::Newline),
    ] {
        assert_eq!(read(&words(misuse)), Asked::Misused(fault), "{misuse:?}");
    }
}

#[test]
fn what_still_hides_a_path_given_to_unhide_is_what_its_lines_leave() {
    let paths = |paths: &[&[u8]]| -> Vec<Vec<u8>> { paths.iter().map(|p| p.to_vec()).collect() };
    let listed = paths(&[b"notes", b"notes/sub", b"a", b"a/b/c", b".env"]);
    let tracked: BTreeSet<Vec<u8>> = paths(&[b".env", b"docs/x.md", b"notes/sub/t.md"])
        .into_iter()
        .collect();
    let given = paths(&[b"notes/sub", b"a", b"a/b/c/d", b".env", b"docs", b"typo"]);
    for (path, expected) in [
        (&b".env"[..], Stays::Tracked),
        // A listed path above outranks the tracked files below.
        (b"notes/sub", Stays::Below(b"notes".to_vec())),
        // `a` goes with this `unhide`; `a/b/c` stays.
        (b"a/b/c/d", Stays::Below(b"a/b/c".to_vec())),
        (b"docs", Stays::TrackedBelow),
        (b"a", Stays::Nothing),
        (b"typo", Stays::Nothing),
        // Below means below a whole component.
        (b"doc", Stays::Nothing),
    ] {
        let found = stays(&listed, &tracked, &given, path);
        assert_eq!(found, expected, "{}", path.escape_ascii());
    }
}

#[test]
fn an_unhide_hint_says_what_became_of_the_line_and_never_that_a_path_is_visible() {
    let removed = |stays| unhidden_line(b"p", true, &stays);
    assert_eq!(
        removed(Stays::Nothing),
        b"p is no longer listed in .gitdupe"
    );
    assert_eq!(
        unhidden_line(b"p", false, &Stays::Nothing),
        b"p is not listed in .gitdupe; there is no line to remove"
    );
    let lines = [
        removed(Stays::Tracked),
        removed(Stays::Below(b"q".to_vec())),
        removed(Stays::TrackedBelow),
        unhidden_line(b"p", false, &Stays::Tracked),
    ];
    assert!(lines[0].ends_with(b"it stays hidden, because the private repository tracks it"));
    assert!(lines[1].ends_with(b"it stays hidden below q, which .gitdupe lists"));
    assert!(lines[2].ends_with(b"files below it that the private repository tracks stay hidden"));
    for line in &lines {
        assert!(
            !line.windows(7).any(|part| part == b"visible"),
            "{}",
            line.escape_ascii()
        );
    }
}

#[test]
fn only_hide_sends_the_developer_to_add_and_never_for_a_tracked_path() {
    let holds = |line: &[u8], part: &[u8]| line.windows(part.len()).any(|at| at == part);
    let hint = |hiding, tracked, standing: Standing| {
        newly_hidden(b"p", tracked, hiding, || standing.clone())
    };
    let present = hint(Hiding::Hide, false, Standing::Something);
    let absent = hint(Hiding::Hide, false, Standing::Nothing);
    let linked = hint(Hiding::Hide, false, Standing::BeyondALink(b"l".to_vec()));
    let by_add = hint(Hiding::Add, false, Standing::Something);
    let tracked = hint(Hiding::Hide, true, Standing::Something);
    for line in [&present, &absent, &linked, &by_add, &tracked] {
        assert!(line.starts_with(b"p "), "{}", line.escape_ascii());
        assert!(
            holds(line, b"'git dupe unhide -- p'"),
            "{}",
            line.escape_ascii()
        );
    }
    assert!(holds(
        &present,
        b"run from the root, 'git dupe add -- p' versions it"
    ));
    assert!(!holds(&present, b"nothing stands"));
    assert!(holds(&absent, b"nothing stands there yet"));
    assert!(holds(
        &absent,
        b"run from the root, 'git dupe add -- p' versions it"
    ));
    // Git's `add` refuses an ignored path without `-f` (G15): both offers say so.
    for line in [&present, &absent] {
        assert!(
            holds(
                line,
                b"or 'git dupe add -f -- p' where Git refuses that as ignored"
            ),
            "{}",
            line.escape_ascii()
        );
    }
    // Git's `add` refuses a path beyond a symbolic link whatever its options (S4).
    assert!(holds(&linked, b"beyond the symbolic link l,"));
    assert!(!holds(&linked, b"git dupe add"));
    assert!(!holds(&by_add, b"git dupe add"));
    assert!(!holds(&tracked, b"git dupe add"));
    assert!(!holds(&tracked, b"now hidden"));
}

#[test]
fn the_route_to_private_names_the_deletion_and_then_add() {
    let holds = |line: &[u8], part: &[u8]| line.windows(part.len()).any(|at| at == part);
    let general = route_to_private(None);
    let whole = route_to_private(Some(b"-p"));
    for line in [&general, &whole] {
        let at = |part: &[u8]| line.windows(part.len()).position(|at| at == part);
        let (Some(rm), Some(add)) = (at(b"'git rm --cached"), at(b"'git dupe add")) else {
            panic!("{}", line.escape_ascii());
        };
        assert!(rm < add, "{}", line.escape_ascii());
        assert!(holds(line, b"a deletion from the project"));
        assert!(holds(line, b"every other clone"));
        assert!(!holds(line, b"--cached' first makes"));
    }
    assert!(general.starts_with(b"such a path becomes private one file at a time: "));
    assert!(whole.starts_with(b"run from the root, 'git rm --cached -- -p' first, "));
    assert!(whole.ends_with(
        b"then 'git dupe add -- -p' makes it private, \
          or 'git dupe add -f -- -p' where the project ignores it"
    ));
    assert!(general.ends_with(b"then 'git dupe add' of it, with -f where the project ignores it"));
}

#[test]
fn a_fault_names_the_word_as_typed_on_one_line() {
    let word = OsStr::from_bytes(b"caf\xe9 *");
    for fault in [
        Fault::Option(word),
        Fault::Pattern(word),
        Fault::Magic(word),
        Fault::Root(word),
        Fault::Outside(word),
    ] {
        assert!(fault.text().starts_with(b"'caf\xe9 *' "), "{fault:?}");
        assert!(!fault.text().contains(&b'\n'), "{fault:?}");
    }
    for fault in [Fault::NoPath, Fault::Newline] {
        assert!(!fault.text().contains(&b'\n'), "{fault:?}");
    }
}
