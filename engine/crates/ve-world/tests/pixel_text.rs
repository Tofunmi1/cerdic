//! A procedural pixel font (no font files): text becomes rectangles, so the HUD is plain geometry. Written before the bodies; do not weaken.
use ve_world::pixel_text::{Layout, layout};

fn lit(text: &str) -> usize {
    layout(text, 1.0).rects.len()
}

#[test]
fn a_glyph_is_five_by_seven_pixels_with_one_pixel_between_letters() {
    let one = layout("I", 1.0);
    assert!(
        (one.height - 7.0).abs() < 1e-6,
        "seven rows: {}",
        one.height
    );
    assert!(
        (one.width - 5.0).abs() < 1e-6,
        "five columns: {}",
        one.width
    );
    let three = layout("III", 1.0);
    assert!(
        (three.width - 17.0).abs() < 1e-6,
        "3 glyphs and 2 gaps: {}",
        three.width
    );
}

#[test]
fn the_size_is_the_edge_of_one_pixel_in_metres() {
    let a = layout("Hello", 1.0);
    let b = layout("Hello", 0.02);
    assert!((b.width - a.width * 0.02).abs() < 1e-5 && (b.height - a.height * 0.02).abs() < 1e-5);
    assert_eq!(a.rects.len(), b.rects.len());
}

#[test]
fn rectangles_stay_inside_the_layout_and_do_not_overlap() {
    let Layout {
        rects,
        width,
        height,
    } = layout("Score 12: ok!", 1.0);
    assert!(!rects.is_empty());
    for r in &rects {
        assert!(
            r[0] >= -1e-6
                && r[1] >= -1e-6
                && r[0] + r[2] <= width + 1e-6
                && r[1] + r[3] <= height + 1e-6,
            "{r:?} outside {width} x {height}"
        );
        assert!(r[2] > 0.0 && r[3] > 0.0);
    }
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            let overlap = a[0] < b[0] + b[2] - 1e-6
                && b[0] < a[0] + a[2] - 1e-6
                && a[1] < b[1] + b[3] - 1e-6
                && b[1] < a[1] + a[3] - 1e-6;
            assert!(!overlap, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn spaces_are_empty_and_different_letters_look_different() {
    assert_eq!(lit(" "), 0);
    assert!(
        (layout(" ", 1.0).width - 5.0).abs() < 1e-6,
        "a space is as wide as a letter"
    );
    assert_ne!(
        layout("O", 1.0).rects,
        layout("0", 1.0).rects,
        "the letter O and the digit zero differ"
    );
    assert_ne!(layout("E", 1.0).rects, layout("F", 1.0).rects);
    assert_ne!(layout("1", 1.0).rects, layout("7", 1.0).rects);
}

#[test]
fn every_printable_ascii_character_has_a_glyph_and_the_rest_show_a_question_mark() {
    for c in (32_u8..=126).map(char::from) {
        let l = layout(&c.to_string(), 1.0);
        assert!((l.width - 5.0).abs() < 1e-6, "{c:?}");
        if c != ' ' {
            assert!(!l.rects.is_empty(), "{c:?} draws something");
        }
    }
    assert_eq!(
        layout("\u{e9}", 1.0).rects,
        layout("?", 1.0).rects,
        "unknown characters draw a question mark"
    );
}

#[test]
fn digits_are_distinct_so_a_score_can_be_read() {
    let digits: Vec<_> = "0123456789"
        .chars()
        .map(|c| layout(&c.to_string(), 1.0).rects)
        .collect();
    for (i, a) in digits.iter().enumerate() {
        for (j, b) in digits.iter().enumerate().skip(i + 1) {
            assert_ne!(a, b, "digits {i} and {j} differ");
        }
    }
}

#[test]
fn layout_is_deterministic_and_the_empty_text_is_empty() {
    assert_eq!(layout("Lives 3", 0.05), layout("Lives 3", 0.05));
    let empty = layout("", 1.0);
    assert!(empty.rects.is_empty() && empty.width == 0.0 && empty.height == 0.0);
}
