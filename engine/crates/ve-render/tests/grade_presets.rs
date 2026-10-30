//! The viewer's colour grades: a few more looks beyond neutral, studio, warm, cool and mono. Each is a name the page can ask for, a grading that
//! sits inside the documented ranges, and a look that differs from every other. Written before the bodies; do not weaken.
use std::str::FromStr;
use ve_render::{
    CONTRAST_RANGE, EXPOSURE_RANGE, GradePreset, Grading, SATURATION_RANGE, TINT_RANGE,
};

const NEW: [&str; 5] = ["sunset", "vivid", "film", "night", "noir"];

#[test]
fn the_new_grades_exist_and_come_after_the_old_ones() {
    let names: Vec<&str> = GradePreset::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(
        &names[..5],
        ["neutral", "studio", "warm", "cool", "mono"],
        "the old menu order is kept"
    );
    assert_eq!(&names[5..], NEW, "the new looks follow, in this order");
}

#[test]
fn every_grade_can_be_asked_for_by_name() {
    for preset in GradePreset::ALL {
        assert_eq!(GradePreset::from_str(preset.name()), Ok(preset));
    }
    assert!(GradePreset::from_str("sepia-2000").is_err());
}

#[test]
fn every_grade_sits_inside_the_documented_ranges() {
    for preset in GradePreset::ALL {
        let grading = Grading::for_preset(preset);
        assert_eq!(
            grading,
            grading.clamped(),
            "{preset}: nothing needs clamping"
        );
        assert!(EXPOSURE_RANGE.0 <= grading.exposure && grading.exposure <= EXPOSURE_RANGE.1);
        assert!(CONTRAST_RANGE.0 <= grading.contrast && grading.contrast <= CONTRAST_RANGE.1);
        assert!(
            SATURATION_RANGE.0 <= grading.saturation && grading.saturation <= SATURATION_RANGE.1
        );
        for channel in grading.tint {
            assert!(TINT_RANGE.0 <= channel && channel <= TINT_RANGE.1);
        }
    }
}

#[test]
fn no_two_grades_are_the_same_look() {
    let all: Vec<Grading> = GradePreset::ALL
        .iter()
        .map(|p| Grading::for_preset(*p))
        .collect();
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn each_new_grade_does_what_its_name_says() {
    let g = |name: &str| Grading::for_preset(GradePreset::from_str(name).unwrap());
    let sunset = g("sunset");
    assert!(
        sunset.tint[0] > sunset.tint[1] && sunset.tint[1] > sunset.tint[2],
        "sunset leans orange: red over green over blue"
    );
    let vivid = g("vivid");
    assert!(
        vivid.saturation >= 1.3 && vivid.contrast > 1.0,
        "vivid pushes colour"
    );
    let film = g("film");
    assert!(
        film.contrast < 1.0 && film.saturation < 1.0,
        "film is soft and slightly faded"
    );
    let night = g("night");
    assert!(
        night.exposure < 0.8 && night.tint[2] > night.tint[0],
        "night is dark and blue"
    );
    let noir = g("noir");
    assert_eq!(noir.saturation, 0.0, "noir has no colour");
    assert!(noir.contrast >= 1.25, "and hard contrast");
    assert!(noir.contrast > g("mono").contrast, "harder than mono");
}

#[test]
fn a_grade_changes_a_colour_the_way_its_controls_say() {
    let mid = [0.5, 0.4, 0.3];
    let neutral = Grading::neutral().grade(mid);
    for name in NEW {
        let graded = Grading::for_preset(GradePreset::from_str(name).unwrap()).grade(mid);
        assert!(
            graded.iter().all(|c| (0.0..=1.0).contains(c)),
            "{name} stays displayable: {graded:?}"
        );
        assert_ne!(graded, neutral, "{name} changes the picture");
    }
}
