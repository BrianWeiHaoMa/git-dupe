use super::*;
use crate::links_and_modes::at_the_top;
use std::path::Component;

fn inside(path: &[u8]) -> Cleaned {
    Cleaned::Inside(path.to_vec())
}

/// Whether this platform reads a drive, as Windows does and Linux and macOS never do.
fn reads_a_drive() -> bool {
    matches!(
        Path::new("C:/x").components().next(),
        Some(Component::Prefix(_))
    )
}

#[test]
fn an_offered_operand_resolves_to_its_path_and_a_pattern_stays_refused() {
    let root = Path::new("/r");
    for path in [&b"notes"[..], b":x", b":/x", b"-x", b"two $words", b"a\\b"] {
        let offered = offered(path);
        assert_eq!(literal(&offered), Ok(()), "{}", path.escape_ascii());
        assert_eq!(rooted_or_literal(&offered), Ok(()));
        assert_eq!(resolve(&offered, b"", root), inside(&with_slashes(path)));
        assert_eq!(
            resolve_rooted(&offered, b"", root),
            inside(&with_slashes(path))
        );
    }
    assert_eq!(offered(b":x"), b"./:x");
    assert_eq!(literal(&offered(b"star*")), Err(NotLiteral::Pattern));
}

#[test]
fn dots_and_repeated_and_trailing_slashes_are_removed() {
    assert_eq!(clean(b"notes"), inside(b"notes"));
    assert_eq!(clean(b"a/./b//"), inside(b"a/b"));
    assert_eq!(clean(b"./a/b/./"), inside(b"a/b"));
    assert_eq!(clean(b"//x"), inside(b"x"));
    assert_eq!(clean(b"a/b/../c"), inside(b"a/c"));
    assert_eq!(clean(b"a/.../b"), inside(b"a/.../b"));
}

#[test]
fn an_absolute_path_is_cleaned_the_same_way_and_dot_dot_at_the_top_is_the_top() {
    for (path, cleaned) in [
        (&b"/w/repo"[..], &b"/w/repo"[..]),
        (b"/w/./repo//sub/../", b"/w/repo"),
        (b"/w/repo/absent/..", b"/w/repo"),
        (b"//w", b"/w"),
        (b"/", b"/"),
        (b"/..", b"/"),
        (b"/../../w/..", b"/"),
        (b"/../w/x.git", b"/w/x.git"),
        (b"/w/...git", b"/w/...git"),
    ] {
        assert_eq!(clean_absolute(path), cleaned, "{}", path.escape_ascii());
    }
}

#[test]
fn the_root_and_what_lies_outside_it_are_told_apart() {
    for root in [&b""[..], b".", b"/", b"a/..", b"./a/b/../.."] {
        assert_eq!(clean(root), Cleaned::Root, "{}", root.escape_ascii());
    }
    assert_eq!(clean(b"../x"), Cleaned::Outside(b"../x".to_vec()));
    assert_eq!(clean(b"a/../../x"), Cleaned::Outside(b"../x".to_vec()));
    assert_eq!(clean(b"../../"), Cleaned::Outside(b"../..".to_vec()));
}

#[test]
fn a_word_holding_a_pattern_byte_or_beginning_with_a_colon_is_not_literal() {
    for word in [&b"a*"[..], b"a?", b"a[1]", b"*", b"docs/[x", b"?"] {
        assert_eq!(
            literal(word),
            Err(NotLiteral::Pattern),
            "{}",
            word.escape_ascii()
        );
    }
    for word in [&b":x"[..], b":/x", b":", b":(top)x", b":*"] {
        assert_eq!(
            literal(word),
            Err(NotLiteral::Magic),
            "{}",
            word.escape_ascii()
        );
    }
    for word in [
        &b"a:b"[..],
        b"-x",
        b"notes/",
        b"",
        b"a]",
        b"caf\xe9",
        b"x!#\\ ",
    ] {
        assert_eq!(literal(word), Ok(()), "{}", word.escape_ascii());
    }
}

#[test]
fn a_word_beginning_with_colon_slash_is_a_literal_path_from_the_root() {
    for word in [
        &b":/"[..],
        b":/notes",
        b":/:x",
        b":/!x",
        b":/a b\xe9",
        b":/../x",
    ] {
        assert_eq!(rooted_or_literal(word), Ok(()), "{}", word.escape_ascii());
    }
    for word in [&b":/a*"[..], b":/x?", b":/[x]"] {
        assert_eq!(
            rooted_or_literal(word),
            Err(NotLiteral::Pattern),
            "{}",
            word.escape_ascii()
        );
    }
    for word in [&b":x"[..], b":", b":(top)x", b"::/x"] {
        assert_eq!(
            rooted_or_literal(word),
            Err(NotLiteral::Magic),
            "{}",
            word.escape_ascii()
        );
    }
    assert_eq!(rooted_or_literal(b"a*"), Err(NotLiteral::Pattern));
    assert_eq!(rooted_or_literal(b"notes/"), Ok(()));

    let root = Path::new("/w/repo");
    assert_eq!(resolve_rooted(b":/", b"sub/", root), Cleaned::Root);
    assert_eq!(
        resolve_rooted(b":/notes/x/", b"sub/", root),
        inside(b"notes/x")
    );
    assert_eq!(resolve_rooted(b":/./a//b", b"", root), inside(b"a/b"));
    assert_eq!(resolve_rooted(b":/:x", b"sub/", root), inside(b":x"));
    assert_eq!(resolve_rooted(b":/sub/..", b"sub/", root), Cleaned::Root);
    assert_eq!(
        resolve_rooted(b":/../x", b"sub/", root),
        Cleaned::Outside(b"../x".to_vec())
    );
    // Read from the root, never as an absolute path.
    assert_eq!(
        resolve_rooted(b":/w/repo/x", b"", root),
        inside(b"w/repo/x")
    );
    assert_eq!(resolve_rooted(b"x", b"sub/", root), inside(b"sub/x"));
    assert_eq!(resolve_rooted(b"..", b"sub/", root), Cleaned::Root);
}

#[test]
fn a_relative_operand_is_joined_to_the_prefix_and_cleaned() {
    let root = Path::new("/w/repo");
    assert_eq!(resolve(b"notes/", b"", root), inside(b"notes"));
    assert_eq!(resolve(b"./a//b/", b"sub/", root), inside(b"sub/a/b"));
    assert_eq!(resolve(b"../x", b"sub/", root), inside(b"x"));
    assert_eq!(resolve(b"-x", b"sub/dir/", root), inside(b"sub/dir/-x"));
    assert_eq!(resolve(b"", b"sub/", root), inside(b"sub"));
    assert_eq!(resolve(b"..", b"sub/", root), Cleaned::Root);
    assert_eq!(resolve(b".", b"", root), Cleaned::Root);
    assert_eq!(resolve(b"", b"", root), Cleaned::Root);
    assert_eq!(resolve(b"a/..", b"", root), Cleaned::Root);
    assert_eq!(
        resolve(b"../x", b"", root),
        Cleaned::Outside(b"../x".to_vec())
    );
    assert_eq!(
        resolve(b"../../x", b"sub/", root),
        Cleaned::Outside(b"../x".to_vec())
    );
}

#[test]
fn an_absolute_operand_is_cleaned_then_made_relative_to_the_root() {
    let root = at_the_top("/w/repo");
    let root = Path::new(bytes::name(&root).unwrap());
    let resolved = |word: &[u8], prefix: &[u8]| resolve(&at_the_top(word), prefix, root);
    assert_eq!(resolved(b"/w/repo/sub/y", b"sub/"), inside(b"sub/y"));
    assert_eq!(resolved(b"/w/repo//a/./b/", b""), inside(b"a/b"));
    assert_eq!(resolved(b"/w/x/../repo/n", b""), inside(b"n"));
    assert_eq!(resolved(b"/../../w/repo/n", b""), inside(b"n"));
    assert_eq!(resolved(b"/..", b""), Cleaned::Outside(at_the_top("/")));
    assert_eq!(resolved(b"/w/repo", b"sub/"), Cleaned::Root);
    assert_eq!(resolved(b"/w/repo/", b""), Cleaned::Root);
    assert_eq!(resolved(b"/w/repo/a/..", b""), Cleaned::Root);
    // Lexically, never through the filesystem: a component is compared whole.
    for outside in [
        &b"/w/repository/x"[..],
        b"/w/rep",
        b"/w",
        b"/",
        b"/w/repo/../x",
    ] {
        assert!(
            matches!(resolved(outside, b""), Cleaned::Outside(_)),
            "{}",
            outside.escape_ascii()
        );
    }
    assert_eq!(
        resolved(b"/w/repo/../x", b""),
        Cleaned::Outside(at_the_top("/w/x"))
    );
    // Every absolute path lies at or below a root that is the top.
    let top = at_the_top("/");
    let top = Path::new(bytes::name(&top).unwrap());
    assert_eq!(resolve(&at_the_top("/etc/x"), b"", top), inside(b"etc/x"));
    assert_eq!(resolve(&at_the_top("/"), b"", top), Cleaned::Root);
    if let Some(root) = bytes::name(b"/caf\xe9") {
        assert_eq!(resolve(b"/caf\xe9/r", b"", Path::new(root)), inside(b"r"));
    }
}

#[test]
fn a_separator_is_what_the_platform_calls_one_and_a_slash_is_written_for_it() {
    if path::is_separator('\\') {
        assert_eq!(clean(b"notes\\x"), inside(b"notes/x"));
        assert_eq!(clean(b".\\notes\\"), inside(b"notes"));
        assert_eq!(clean(b"\\notes"), inside(b"notes"));
        assert_eq!(clean(b"a\\..\\b"), inside(b"b"));
        assert_eq!(
            resolve(b".\\notes\\", b"sub/", Path::new("C:/w")),
            inside(b"sub/notes")
        );
    } else {
        assert_eq!(clean(b"notes\\x"), inside(b"notes\\x"));
        assert_eq!(clean(b".\\notes\\"), inside(b".\\notes\\"));
        assert_eq!(clean(b"\\notes"), inside(b"\\notes"));
    }
    // A byte above ASCII is no separator whatever `char::from` makes of it.
    assert_eq!(with_slashes(b"caf\xc3\xa9/\xff"), b"caf\xc3\xa9/\xff");
}

#[test]
fn a_path_that_begins_with_a_drive_or_a_unc_name_is_outside_where_the_platform_reads_one() {
    for (path, on_windows, elsewhere) in [
        (&b"C:/x"[..], &b"C:/x"[..], &b"C:/x"[..]),
        (b"c:x", b"c:x", b"c:x"),
        (b"C:\\x", b"C:/x", b"C:\\x"),
        (b"/C:/x", b"C:/x", b"C:/x"),
        (
            b"\\\\server\\share\\x",
            b"//server/share/x",
            b"\\\\server\\share\\x",
        ),
    ] {
        let expected = if reads_a_drive() {
            Cleaned::Outside(on_windows.to_vec())
        } else {
            inside(elsewhere)
        };
        assert_eq!(clean(path), expected, "{}", path.escape_ascii());
    }
    // A byte that is no name further on hides no drive.
    assert_eq!(
        matches!(clean(b"C:/caf\xe9"), Cleaned::Outside(_)),
        reads_a_drive()
    );
    // Only the first component: nowhere else does a colon mean a drive.
    assert_eq!(clean(b"a/C:/x"), inside(b"a/C:/x"));
    assert_eq!(clean(b"notes:x"), inside(b"notes:x"));
}

#[test]
fn an_absolute_word_is_what_the_platform_calls_absolute_and_a_word_without_is_refused() {
    let drive = Path::new("C:/proj");
    if Path::new("C:/proj").is_absolute() {
        for word in [
            &b"C:\\proj\\notes"[..],
            b"C:/proj/notes",
            b"c:/proj/notes",
            b"c:\\proj\\notes",
            b"C:/proj/./x/../notes/",
            b"C://proj//notes",
            b"c:\\\\proj\\x\\..\\notes",
        ] {
            assert_eq!(resolve(word, b"", drive), inside(b"notes"), "{word:?}");
        }
        assert_eq!(resolve(b"c:\\proj", b"sub/", drive), Cleaned::Root);
        assert!(matches!(
            resolve(b"D:/proj/notes", b"", drive),
            Cleaned::Outside(_)
        ));
        assert!(matches!(
            resolve(b"C:/PROJ/notes", b"", drive),
            Cleaned::Outside(_)
        ));
        assert_eq!(
            resolve(b"C:/notes", b"", Path::new("C:/")),
            inside(b"notes")
        );
        assert_eq!(resolve(b"c:\\", b"", Path::new("C:/")), Cleaned::Root);
        assert_eq!(
            resolve(b"//server/share/x", b"", Path::new("//server/share")),
            inside(b"x")
        );
        assert!(matches!(
            resolve(b"\\\\Server\\share\\x", b"", Path::new("//server/share")),
            Cleaned::Outside(_)
        ));
    } else {
        assert_eq!(
            resolve(b"/proj/notes", b"", Path::new("/proj")),
            inside(b"notes")
        );
        assert_eq!(
            resolve(b"C:/proj/notes", b"", drive),
            inside(b"C:/proj/notes")
        );
    }
    // A drive and no root, or a root and no drive, joined to no prefix.
    for word in [&b"C:notes"[..], b"\\notes"] {
        let expected = if reads_a_drive() {
            Cleaned::Outside(with_slashes(word))
        } else {
            inside(&[b"sub/", word].concat())
        };
        assert_eq!(resolve(word, b"sub/", drive), expected, "{word:?}");
    }
    assert!(matches!(
        resolve(b"/notes", b"sub/", drive),
        Cleaned::Outside(_)
    ));
    // Only a drive letter is folded: another case of any other byte is another path.
    assert!(matches!(
        resolve(b"/PROJ/notes", b"", Path::new("/proj")),
        Cleaned::Outside(_)
    ));
}

#[test]
fn the_drive_letter_alone_is_folded_in_the_lexical_form_of_an_absolute_path() {
    if reads_a_drive() {
        for (path, cleaned) in [
            (&b"c:\\w\\repo\\absent\\.."[..], &b"C:/w/repo"[..]),
            (b"C:/w/repo", b"C:/w/repo"),
            (b"c:/W/Repo", b"C:/W/Repo"),
            (b"C:/..", b"C:/"),
            (b"c:\\", b"C:/"),
            (b"//server/share/x/../y", b"//server/share/y"),
            // What follows the drive is below the top, whatever it looks like alone.
            (b"C://w//x/../y", b"C:/w/y"),
            (b"C:/D:/x", b"C:/D:/x"),
        ] {
            assert_eq!(clean_absolute(path), cleaned, "{}", path.escape_ascii());
        }
    } else {
        assert_eq!(clean_absolute(b"/c:/W/x"), b"/c:/W/x");
    }
}

#[test]
fn an_ancestor_is_a_whole_leading_component_path_and_the_root_is_none() {
    let above = |path: &[u8]| ancestors(path).map(<[u8]>::to_vec).collect::<Vec<_>>();
    assert_eq!(above(b"a/b/c"), [b"a".to_vec(), b"a/b".to_vec()]);
    assert_eq!(
        above(b"docs/design.md/x"),
        [b"docs".to_vec(), b"docs/design.md".to_vec()]
    );
    assert!(above(b"a").is_empty());
    assert!(above(b"").is_empty());

    assert!(at_or_below(b"notes/a/b", b"notes"));
    assert!(at_or_below(b"notes", b"notes"));
    // Lexical: a file's path can have paths below it.
    assert!(at_or_below(b"docs/design.md/x", b"docs/design.md"));
    for beside in [&b"notesx"[..], b"notes-old", b"notes.d", b"note"] {
        assert!(!at_or_below(beside, b"notes"), "{}", beside.escape_ascii());
    }
    assert!(!at_or_below(b"notes", b"notes/a"));
    // The root is at or below nothing but itself, and no path lies below it here.
    assert!(!at_or_below(b"a", b""));
    assert!(!at_or_below(b"", b"a"));
    assert!(at_or_below(b"", b""));
}

#[test]
fn a_set_holds_a_path_at_or_below_another_by_whole_components() {
    let paths: BTreeSet<Vec<u8>> = [&b"notes-old/x"[..], b"notes.d", b"notes/a/b", b"z"]
        .map(<[u8]>::to_vec)
        .into();
    assert!(any_at_or_below(&paths, b"notes"));
    assert!(any_at_or_below(&paths, b"notes/a"));
    assert!(any_at_or_below(&paths, b"notes/a/b"));
    assert!(any_at_or_below(&paths, b"z"));
    // `notes-old` and `notes.d` sort between `notes` and `notes/` and lie beside it.
    let beside: BTreeSet<Vec<u8>> = [&b"notes-old/x"[..], b"notes.d"].map(<[u8]>::to_vec).into();
    assert!(!any_at_or_below(&beside, b"notes"));
    assert!(!any_at_or_below(&paths, b"notes/a/b/c"));
    assert!(!any_at_or_below(&paths, b"note"));
    assert!(!any_at_or_below(&paths, b""));
    // Below excludes the path itself.
    assert!(any_below(&paths, b"notes/a"));
    assert!(!any_below(&paths, b"notes/a/b"));
    assert!(!any_below(&paths, b"z"));
}

#[test]
fn every_other_byte_is_part_of_the_path() {
    assert_eq!(clean(b" trail \r"), inside(b" trail \r"));
    assert_eq!(clean(b"caf\xe9/#x/*"), inside(b"caf\xe9/#x/*"));
    assert_eq!(clean(b"a\\b"), inside(&with_slashes(b"a\\b")));
}
