use super::*;

const MAIN: Worktree = Worktree::Main;
const AGENT: Worktree = Worktree::Linked(b"agent");

fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
    paths.iter().map(|path| path.to_vec()).collect()
}

/// The file with `worktree`'s region set and every other region kept, as when every other
/// worktree holds its private repository.
fn set_keeping_all(found: &Sections, worktree: Worktree, paths: &[Vec<u8>]) -> Vec<u8> {
    found.set(worktree, paths, &vec![true; found.others.len()])
}

/// The file without the region and with every other region kept.
fn removed_keeping_all(found: &Sections) -> Vec<u8> {
    found.removed(&vec![true; found.others.len()])
}

#[test]
fn a_file_without_a_region_is_all_the_users_and_the_region_is_appended() {
    for (file, composed) in [
        (
            &b""[..],
            &b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"[..],
        ),
        (
            b"*.o\n",
            b"*.o\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
        ),
        // The begin marker starts a line; the user's bytes stay a prefix.
        (
            b"*.o",
            b"*.o\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
        ),
        (
            b"a\r\n",
            b"a\r\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
        ),
    ] {
        let found = split(file, MAIN);
        assert_eq!(found.before, file);
        assert_eq!(found.rules, None);
        assert_eq!(
            set_keeping_all(&found, MAIN, &paths(&[b".gitdupe"])),
            composed
        );
    }
}

#[test]
fn a_region_is_rewritten_where_it_stands_with_the_users_bytes_around_it() {
    let file = b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/old\n# END git-dupe\nafter\nlast";
    let found = split(file, MAIN);
    assert_eq!(found.before, b"*.o\n");
    assert_eq!(found.rules, Some(vec![&b"/.gitdupe"[..], b"/old"]));
    assert_eq!(found.after, b"after\nlast");
    assert_eq!(
        set_keeping_all(&found, MAIN, &paths(&[b".gitdupe", b"new"])),
        b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/new\n# END git-dupe\nafter\nlast"
    );
}

#[test]
fn a_begin_marker_without_an_end_marker_runs_to_the_end_of_the_file() {
    for file in [
        &b"x\n# BEGIN git-dupe\n/.gitdupe\n/old\n"[..],
        b"x\n# BEGIN git-dupe\n/.gitdupe\n/old",
    ] {
        let found = split(file, MAIN);
        assert_eq!(found.before, b"x\n");
        assert_eq!(found.rules, Some(vec![&b"/.gitdupe"[..], b"/old"]));
        assert_eq!(found.after, b"");
        assert_eq!(
            set_keeping_all(&found, MAIN, &paths(&[b".gitdupe"])),
            b"x\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"
        );
    }
    // A linked worktree's the same way, and its markers are its own.
    let found = split(b"x\n# BEGIN git-dupe worktree agent\n/a", AGENT);
    assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
    assert_eq!(
        set_keeping_all(&found, AGENT, &paths(&[b"b"])),
        b"x\n# BEGIN git-dupe worktree agent\n/b\n# END git-dupe worktree agent\n"
    );
}

#[test]
fn the_first_own_begin_marker_and_the_first_end_marker_after_it_bound_the_region() {
    let file =
        b"# END git-dupe\n# BEGIN git-dupe\n/a\n# END git-dupe\n# BEGIN git-dupe\n# END git-dupe\n";
    let found = split(file, MAIN);
    assert_eq!(found.before, b"# END git-dupe\n");
    assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
    assert_eq!(found.after, b"# BEGIN git-dupe\n# END git-dupe\n");
    let file = b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
                 # BEGIN git-dupe worktree agent\n/b\n# END git-dupe worktree agent\n";
    let found = split(file, AGENT);
    assert_eq!(found.before, b"");
    assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
    assert_eq!(
        found.after,
        b"# BEGIN git-dupe worktree agent\n/b\n# END git-dupe worktree agent\n"
    );
}

#[test]
fn an_end_marker_of_either_form_ends_the_region() {
    // Another worktree's end marker, and the main one's, end a linked worktree's region,
    // and the main region the other way round; whatever follows is copied.
    for (file, worktree, rules, after) in [
        (
            &b"# BEGIN git-dupe\n/a\n# END git-dupe worktree agent\nuser\n"[..],
            MAIN,
            vec![&b"/a"[..]],
            &b"user\n"[..],
        ),
        (
            b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe\nuser\n",
            AGENT,
            vec![b"/a"],
            b"user\n",
        ),
        (
            b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree other\nuser\n",
            AGENT,
            vec![b"/a"],
            b"user\n",
        ),
        // A begin marker inside the region is one of its lines; the first end marker
        // after the own begin marker ends it all the same.
        (
            b"# BEGIN git-dupe\n/a\n# BEGIN git-dupe worktree agent\n/b\n\
              # END git-dupe worktree agent\nuser\n",
            MAIN,
            vec![b"/a", b"# BEGIN git-dupe worktree agent", b"/b"],
            b"user\n",
        ),
    ] {
        let found = split(file, worktree);
        assert_eq!(found.before, b"", "{}", file.escape_ascii());
        assert_eq!(found.rules, Some(rules), "{}", file.escape_ascii());
        assert_eq!(found.after, after, "{}", file.escape_ascii());
    }
}

#[test]
fn a_marker_is_a_whole_line() {
    for file in [
        &b"  # BEGIN git-dupe\n/a\n"[..],
        b"# BEGIN git-dupe \n/a\n",
        b"# BEGIN git-dupe\r\n/a\n",
        // A linked worktree's marker is not the main one's, whatever its name.
        b"# BEGIN git-dupe worktree agent\n/a\n",
        b"# BEGIN git-dupe worktree\n/a\n",
        b"# BEGIN git-dupe worktree \n/a\n",
    ] {
        assert_eq!(split(file, MAIN).rules, None, "{}", file.escape_ascii());
    }
    let found = split(b"# BEGIN git-dupe\n/a\n# END git-dupe\r\nx\n", MAIN);
    assert_eq!(
        found.rules,
        Some(vec![&b"/a"[..], b"# END git-dupe\r", b"x"])
    );
    // A name is the whole rest of the line: one that only begins with this one's, or
    // has a trailing byte, is another worktree's.
    for file in [
        &b"# BEGIN git-dupe worktree agent2\n/a\n"[..],
        b"# BEGIN git-dupe worktree agen\n/a\n",
        b"# BEGIN git-dupe worktree agent \n/a\n",
        b"# BEGIN git-dupe worktree agent\r\n/a\n",
        b"# BEGIN git-dupe\n/a\n",
        b"# BEGIN git-dupe  worktree agent\n/a\n",
    ] {
        assert_eq!(split(file, AGENT).rules, None, "{}", file.escape_ascii());
    }
    // Lines that are no marker of either form end nothing.
    let found = split(
        b"# BEGIN git-dupe\n/a\n# END git-dupe worktree\n# END git-dupe worktree \n\
          # END git-dupe \n#END git-dupe\n",
        MAIN,
    );
    assert_eq!(found.rules.map(|rules| rules.len()), Some(5));
    assert_eq!(found.after, b"");
}

#[test]
fn an_end_marker_without_a_final_newline_ends_the_file() {
    let found = split(b"# BEGIN git-dupe\n/a\n# END git-dupe", MAIN);
    assert_eq!(found.before, b"");
    assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
    assert_eq!(found.after, b"");
    let found = split(
        b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe",
        AGENT,
    );
    assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
    assert_eq!(found.after, b"");
}

/// The user's text, the main region, and two linked worktrees' regions, in this order.
const THREE: &[u8] = b"user before\n\
    # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
    between\n\
    # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
    # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
    user after";

#[test]
fn each_worktrees_region_is_read_set_and_removed_alone() {
    let cafe = Worktree::Linked(b"caf\xe9");
    assert_eq!(split(THREE, MAIN).paths(), paths(&[b".gitdupe", b"main"]));
    assert_eq!(split(THREE, AGENT).paths(), paths(&[b"a"]));
    assert_eq!(split(THREE, cafe).paths(), paths(&[b"caf\xe9"]));
    assert_eq!(split(THREE, Worktree::Linked(b"gone")).rules, None);

    let set = paths(&[b".gitdupe", b"n\xffew"]);
    assert_eq!(
        set_keeping_all(&split(THREE, MAIN), MAIN, &set),
        b"user before\n\
          # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
          between\n\
          # BEGIN git-dupe\n/.gitdupe\n/n\xffew\n# END git-dupe\n\
          # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
          user after"
    );
    assert_eq!(
        set_keeping_all(&split(THREE, AGENT), AGENT, &set),
        b"user before\n\
          # BEGIN git-dupe worktree agent\n/.gitdupe\n/n\xffew\n# END git-dupe worktree agent\n\
          between\n\
          # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
          # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
          user after"
    );
    assert_eq!(
        set_keeping_all(&split(THREE, cafe), cafe, &set),
        b"user before\n\
          # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
          between\n\
          # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
          # BEGIN git-dupe worktree caf\xe9\n/.gitdupe\n/n\xffew\n# END git-dupe worktree caf\xe9\n\
          user after"
    );

    assert_eq!(
        removed_keeping_all(&split(THREE, MAIN)),
        b"user before\n\
          # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
          between\n\
          # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
          user after"
    );
    assert_eq!(
        removed_keeping_all(&split(THREE, AGENT)),
        b"user before\n\
          between\n\
          # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
          # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
          user after"
    );
}

#[test]
fn a_missing_region_is_appended_after_every_other_one() {
    // After the other regions and the user's last line, which had no newline.
    let file = b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\nlast";
    assert_eq!(
        set_keeping_all(&split(file, MAIN), MAIN, &paths(&[b".gitdupe"])),
        b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\nlast\n\
          # BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"
    );
    // Removing a region that is not there is the file as it was.
    assert_eq!(removed_keeping_all(&split(file, MAIN)), file);
    assert_eq!(
        removed_keeping_all(&split(THREE, Worktree::Linked(b"gone"))),
        THREE
    );
}

#[test]
fn deleting_the_region_keeps_the_users_bytes_around_it_and_nothing_else() {
    let file = b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\nafter\nlast";
    assert_eq!(removed_keeping_all(&split(file, MAIN)), b"*.o\nafter\nlast");
    assert_eq!(
        removed_keeping_all(&split(
            b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            MAIN
        )),
        b""
    );
    // A begin marker without an end marker: everything from it goes.
    assert_eq!(
        removed_keeping_all(&split(
            b"x\n# BEGIN git-dupe\n/a\n# END git-dupe \ny\n",
            MAIN
        )),
        b"x\n"
    );
    // The newline composing put before the begin marker stays: it cannot be told from
    // the user's own.
    let composed = set_keeping_all(&split(b"*.o", MAIN), MAIN, &paths(&[b".gitdupe"]));
    assert_eq!(removed_keeping_all(&split(&composed, MAIN)), b"*.o\n");
}

#[test]
fn composing_what_was_composed_changes_nothing() {
    let region = paths(&[b".gitdupe", b"has space", b"cr\rx"]);
    for worktree in [MAIN, AGENT, Worktree::Linked(b"caf\xe9")] {
        let once = set_keeping_all(&split(THREE, worktree), worktree, &region);
        assert_eq!(
            set_keeping_all(&split(&once, worktree), worktree, &region),
            once
        );
        assert_eq!(split(&once, worktree).paths(), region);
    }
}

#[test]
fn every_other_region_is_found_around_this_one_with_its_worktree_and_rules() {
    let cafe = Worktree::Linked(b"caf\xe9");
    let found = split(THREE, MAIN);
    let others: Vec<(Worktree, Vec<Vec<u8>>)> = found
        .others
        .iter()
        .map(|other| (other.worktree, other.paths()))
        .collect();
    assert_eq!(
        others,
        [(AGENT, paths(&[b"a"])), (cafe, paths(&[b"caf\xe9"]))]
    );
    let found = split(THREE, AGENT);
    let others: Vec<Worktree> = found.others.iter().map(|other| other.worktree).collect();
    assert_eq!(others, [MAIN, cafe]);
    // Without a region of its own, every region in the file is another's.
    let found = split(THREE, Worktree::Linked(b"gone"));
    assert_eq!(found.others.len(), 3);
}

#[test]
fn a_region_left_out_takes_its_whole_lines_and_nothing_else() {
    let cafe = Worktree::Linked(b"caf\xe9");
    let found = split(THREE, MAIN);
    // AGENT's region gone, CAFE's kept.
    assert_eq!(
        found.set(MAIN, &paths(&[b".gitdupe"]), &[false, true]),
        b"user before\n\
          between\n\
          # BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n\
          # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
          user after"
    );
    assert_eq!(
        found.removed(&[true, false]),
        b"user before\n\
          # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
          between\n\
          user after"
    );
    // Both gone, with no region of this worktree's: only the user's bytes stay.
    let found = split(THREE, cafe);
    assert_eq!(
        found.removed(&[false, false]),
        b"user before\nbetween\nuser after"
    );
    // A region at the end without a final newline goes to the end of the file, and the
    // region set after the user's text starts a line of its own.
    let file = b"user\n# BEGIN git-dupe worktree gone\n/x\n# END git-dupe worktree gone";
    let found = split(file, MAIN);
    assert_eq!(
        found.set(MAIN, &paths(&[b".gitdupe"]), &[false]),
        b"user\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"
    );
    // Kept, every byte stays, and composing it again changes nothing.
    let once = found.set(MAIN, &paths(&[b".gitdupe"]), &[true]);
    assert!(once.starts_with(file));
    let again = split(&once, MAIN);
    assert_eq!(again.set(MAIN, &paths(&[b".gitdupe"]), &[true]), once);
}

#[test]
fn another_region_is_bounded_as_this_one_is_and_never_overlaps_it() {
    // Without an end marker, another region runs to this worktree's begin marker, or to
    // the end of the file after it; a begin marker inside it is one of its lines.
    let file = b"# BEGIN git-dupe worktree agent\n/a\n# BEGIN git-dupe worktree other\n/o\n\
                 # BEGIN git-dupe\n/m\n# END git-dupe\n\
                 # BEGIN git-dupe worktree late\n/l\n";
    let found = split(file, MAIN);
    let others: Vec<(Worktree, Vec<&[u8]>)> = found
        .others
        .iter()
        .map(|other| (other.worktree, other.rules.clone()))
        .collect();
    assert_eq!(
        others,
        [
            (
                AGENT,
                vec![&b"/a"[..], b"# BEGIN git-dupe worktree other", b"/o"]
            ),
            (Worktree::Linked(b"late"), vec![&b"/l"[..]]),
        ]
    );
    assert_eq!(
        found.set(MAIN, &paths(&[b"m"]), &[false, false]),
        b"# BEGIN git-dupe\n/m\n# END git-dupe\n"
    );
    // An end marker of any form ends it; one that ends no region is the user's line.
    let file = b"# END git-dupe worktree x\n\
                 # BEGIN git-dupe worktree agent\n/a\n# END git-dupe\nuser\n";
    let found = split(file, Worktree::Linked(b"b"));
    assert_eq!(found.others.len(), 1);
    assert_eq!(
        found.removed(&[false]),
        b"# END git-dupe worktree x\nuser\n"
    );
    // A second region of this worktree's own markers is no other worktree's: it is
    // copied as the user's text is.
    let file = b"# BEGIN git-dupe\n/a\n# END git-dupe\n# BEGIN git-dupe\n/b\n# END git-dupe\n\
                 # BEGIN git-dupe worktree agent\n/c\n# END git-dupe worktree agent\n";
    let found = split(file, MAIN);
    assert_eq!(found.others.len(), 1);
    assert_eq!(found.others[0].worktree, AGENT);
    assert_eq!(
        found.set(MAIN, &paths(&[b"a"]), &[false]),
        b"# BEGIN git-dupe\n/a\n# END git-dupe\n# BEGIN git-dupe\n/b\n# END git-dupe\n"
    );
}
