//! Unit tests for the [`TextInput`](super::TextInput) primitive: char-boundary safety, caret
//! movement, mid-buffer insert/delete, and the single-line / multiline render math.

use super::{TextInput, Viewport};

#[test]
fn default_is_empty_with_caret_at_zero() {
    let input = TextInput::default();
    assert_eq!(input.as_str(), "");
    assert_eq!(input.caret(), 0);
    assert!(input.is_empty());
}

#[test]
fn new_seeds_caret_at_end() {
    let input = TextInput::new("hello");
    assert_eq!(input.as_str(), "hello");
    assert_eq!(input.caret(), 5);
}

#[test]
fn insert_at_end_appends() {
    let mut input = TextInput::new("ab");
    input.insert_char('c');
    assert_eq!(input.as_str(), "abc");
    assert_eq!(input.caret(), 3);
}

#[test]
fn insert_mid_buffer() {
    let mut input = TextInput::new("ac");
    input.move_left(); // caret before 'c'
    input.insert_char('b');
    assert_eq!(input.as_str(), "abc");
    assert_eq!(input.caret(), 2);
}

#[test]
fn insert_at_start() {
    let mut input = TextInput::new("bc");
    input.home();
    input.insert_char('a');
    assert_eq!(input.as_str(), "abc");
    assert_eq!(input.caret(), 1);
}

#[test]
fn backspace_at_start_is_noop() {
    let mut input = TextInput::new("ab");
    input.home();
    input.backspace();
    assert_eq!(input.as_str(), "ab");
    assert_eq!(input.caret(), 0);
}

#[test]
fn backspace_mid_buffer() {
    let mut input = TextInput::new("abc");
    input.move_left(); // caret before 'c'
    input.backspace(); // remove 'b'
    assert_eq!(input.as_str(), "ac");
    assert_eq!(input.caret(), 1);
}

#[test]
fn delete_forward_mid_buffer() {
    let mut input = TextInput::new("abc");
    input.home();
    input.delete(); // remove 'a'
    assert_eq!(input.as_str(), "bc");
    assert_eq!(input.caret(), 0);
}

#[test]
fn delete_at_end_is_noop() {
    let mut input = TextInput::new("ab");
    input.delete();
    assert_eq!(input.as_str(), "ab");
    assert_eq!(input.caret(), 2);
}

#[test]
fn multibyte_insert_and_delete_stay_on_boundaries() {
    // Each of these is a multi-byte char; caret math must be char-based, not byte-based.
    let mut input = TextInput::new("áé");
    input.home();
    input.move_right(); // caret between 'á' and 'é'
    input.insert_char('ñ');
    assert_eq!(input.as_str(), "áñé");
    assert_eq!(input.caret(), 2);
    input.backspace(); // remove 'ñ'
    assert_eq!(input.as_str(), "áé");
    assert_eq!(input.caret(), 1);
    input.delete(); // forward-delete 'é'
    assert_eq!(input.as_str(), "á");
    assert_eq!(input.caret(), 1);
}

#[test]
fn move_left_right_clamp_at_ends() {
    let mut input = TextInput::new("ab");
    input.move_right(); // already at end
    assert_eq!(input.caret(), 2);
    input.home();
    input.move_left(); // already at start
    assert_eq!(input.caret(), 0);
}

#[test]
fn home_and_end_are_line_local_in_multiline() {
    let mut input = TextInput::new("ab\ncd");
    // caret at end (after 'd', line 1)
    input.home();
    assert_eq!(input.caret(), 3); // start of "cd"
    input.end();
    assert_eq!(input.caret(), 5); // end of "cd"
}

#[test]
fn move_up_down_preserve_column() {
    let mut input = TextInput::new("abcd\nef\nghij");
    // caret at very end (line 2, col 4)
    input.move_up(); // to line 1 "ef", clamped to col 2
    assert_eq!(input.caret(), 7); // "abcd\nef" -> index 7 is end of "ef"
    input.move_up(); // to line 0 "abcd", restore toward original column via clamp
    // column from line 1 was 2 (clamped), so lands at col 2 of line 0
    assert_eq!(input.caret(), 2);
}

#[test]
fn move_up_at_first_line_goes_to_start() {
    let mut input = TextInput::new("abc");
    input.move_up();
    assert_eq!(input.caret(), 0);
}

#[test]
fn move_down_at_last_line_goes_to_end() {
    let mut input = TextInput::new("abc");
    input.home();
    input.move_down();
    assert_eq!(input.caret(), 3);
}

#[test]
fn field_view_no_scroll_when_short() {
    let input = TextInput::new("abc");
    let (visible, col) = input.field_view(10);
    assert_eq!(visible, "abc");
    assert_eq!(col, 3);
}

#[test]
fn field_view_scrolls_to_keep_caret_visible() {
    let input = TextInput::new("abcdefgh"); // caret at 8
    let (visible, col) = input.field_view(4);
    // caret past width: scroll = 8 - 4 + 1 = 5, visible = chars[5..9] = "fgh"
    assert_eq!(visible, "fgh");
    assert_eq!(col, 3);
}

#[test]
fn field_view_zero_width_is_empty() {
    let input = TextInput::new("abc");
    let (visible, col) = input.field_view(0);
    assert_eq!(visible, "");
    assert_eq!(col, 0);
}

#[test]
fn viewport_single_short_line() {
    let input = TextInput::new("hello");
    let vp = input.viewport(20, 3);
    assert_eq!(
        vp,
        Viewport {
            lines: vec!["hello".to_owned()],
            caret_row: 0,
            caret_col: 5,
        }
    );
}

#[test]
fn viewport_honours_newlines() {
    let mut input = TextInput::new("ab\ncd");
    input.home(); // caret at start of "cd" (line 1, col 0)
    let vp = input.viewport(20, 3);
    assert_eq!(vp.lines, vec!["ab".to_owned(), "cd".to_owned()]);
    assert_eq!(vp.caret_row, 1);
    assert_eq!(vp.caret_col, 0);
}

#[test]
fn viewport_hard_wraps_long_line() {
    let input = TextInput::new("abcdefg"); // caret at 7
    let vp = input.viewport(3, 5);
    // wrapped: "abc","def","g" ; caret at col 7 -> row 2, col 1
    assert_eq!(
        vp.lines,
        vec!["abc".to_owned(), "def".to_owned(), "g".to_owned()]
    );
    assert_eq!(vp.caret_row, 2);
    assert_eq!(vp.caret_col, 1);
}

#[test]
fn viewport_caret_at_exact_wrap_boundary_shows_trailing_row() {
    let input = TextInput::new("abcdef"); // len 6, caret at 6
    let vp = input.viewport(3, 5);
    // "abc","def" then caret parks on an empty trailing row (row 2, col 0)
    assert_eq!(
        vp.lines,
        vec!["abc".to_owned(), "def".to_owned(), String::new()]
    );
    assert_eq!(vp.caret_row, 2);
    assert_eq!(vp.caret_col, 0);
}

#[test]
fn viewport_scrolls_to_keep_caret_on_last_visible_row() {
    // Five logical lines; viewport height 2 keeps the caret line (line 4) on the last row.
    let input = TextInput::new("l0\nl1\nl2\nl3\nl4"); // caret at end (line 4)
    let vp = input.viewport(10, 2);
    // abs caret row = 4, scroll = 4 - (2-1) = 3, visible = rows 3,4
    assert_eq!(vp.lines, vec!["l3".to_owned(), "l4".to_owned()]);
    assert_eq!(vp.caret_row, 1);
    assert_eq!(vp.caret_col, 2);
}

#[test]
fn viewport_no_scroll_when_caret_within_first_screen() {
    let mut input = TextInput::new("l0\nl1\nl2\nl3\nl4");
    // caret starts on line 4 (end); move up to line 1
    input.move_up(); // line 3
    input.move_up(); // line 2
    input.move_up(); // line 1
    let vp = input.viewport(10, 3);
    assert_eq!(
        vp.lines,
        vec!["l0".to_owned(), "l1".to_owned(), "l2".to_owned()]
    );
    // caret line 1 within [0,3): no scroll
    assert_eq!(vp.caret_row, 1);
}

#[test]
fn viewport_empty_buffer() {
    let input = TextInput::default();
    let vp = input.viewport(10, 3);
    assert_eq!(vp.lines, vec![String::new()]);
    assert_eq!(vp.caret_row, 0);
    assert_eq!(vp.caret_col, 0);
}
