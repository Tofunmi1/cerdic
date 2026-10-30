//! Measuring a body and fitting named measurements to phenotype parameters: the editable part of the human (ADR 0055). The model is linear in its blend shapes
//! for fixed phenotype weights, so a bounded, regularised least-squares fit over a few parameters is cheap and exact enough. Ranges below are the adult
//! values of the ANSUR II survey, widened a little: the point is plausible people, not a particular one.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use std::sync::OnceLock;

use ve_human::{FitOptions, Human, Measure, Pack, Params, Target, fit, measure};

fn human() -> &'static Human {
    static HUMAN: OnceLock<Human> = OnceLock::new();
    HUMAN.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
        Human::new(Pack::from_zstd(&std::fs::read(path).unwrap()).unwrap())
    })
}

fn adult(pairs: &[(&str, f32)]) -> Params {
    let mut p = Params::default();
    p.phenotype.insert("age".to_owned(), 0.667);
    for (name, value) in pairs {
        p.phenotype.insert((*name).to_owned(), *value);
    }
    p
}

fn value(params: &Params, what: Measure) -> f64 {
    measure(human(), &human().evaluate(params).unwrap(), &what).unwrap()
}

#[test]
fn an_adult_has_plausible_measurements() {
    let man = adult(&[("gender", 0.0)]);
    let woman = adult(&[("gender", 1.0)]);
    for (what, low, high) in [
        (Measure::Stature, 1.55, 1.95),
        (Measure::Mass, 45.0, 110.0),
        (Measure::ShoulderWidth, 0.25, 0.50),
        (Measure::ArmLength, 0.45, 0.80),
        (Measure::LegLength, 0.70, 1.10),
        (Measure::NeckCircumference, 0.28, 0.45),
        (Measure::ChestCircumference, 0.75, 1.30),
        (Measure::WaistCircumference, 0.60, 1.15),
        (Measure::HipCircumference, 0.80, 1.30),
        (Measure::HeadWidth, 0.12, 0.19),
    ] {
        for (who, params) in [("man", &man), ("woman", &woman)] {
            let v = value(params, what.clone());
            assert!(
                (low..=high).contains(&v),
                "{who} {what:?}: {v} not in {low}..{high}"
            );
        }
    }
    assert!(
        value(&man, Measure::Stature) > value(&woman, Measure::Stature),
        "the male prototype is taller"
    );
}

#[test]
fn measurements_follow_the_sliders_in_the_right_direction() {
    let base = adult(&[]);
    let heavy = adult(&[("weight", 1.0)]);
    let thin = adult(&[("weight", 0.0)]);
    // Measured against the reference model's own torso sections: the weight slider moves a waist by about 8 to 10 cm from one end to the other.
    assert!(
        value(&heavy, Measure::WaistCircumference)
            > value(&thin, Measure::WaistCircumference) + 0.06
    );
    let (heavy_mass, thin_mass) = (value(&heavy, Measure::Mass), value(&thin, Measure::Mass));
    assert!(
        heavy_mass > thin_mass + 8.0,
        "mass {thin_mass} to {heavy_mass}"
    );
    assert!(
        value(&adult(&[("height", 1.0)]), Measure::Stature)
            > value(&adult(&[("height", 0.0)]), Measure::Stature) + 0.05
    );
    let mut wide = base.clone();
    wide.local.insert("measure-waist-circ-incr".to_owned(), 1.0);
    assert!(
        value(&wide, Measure::WaistCircumference)
            > value(&base, Measure::WaistCircumference) + 0.03
    );
}

#[test]
fn measure_names_round_trip_and_unknown_ones_list_the_valid_names() {
    for what in [
        Measure::Stature,
        Measure::Mass,
        Measure::WaistCircumference,
        Measure::HeadWidth,
    ] {
        assert_eq!(Measure::parse(what.name()).unwrap(), what);
    }
    assert_eq!(
        Measure::parse("waist_circumference").unwrap(),
        Measure::WaistCircumference
    );
    let error = format!("{:?}", Measure::parse("waste_circumference").unwrap_err());
    assert!(
        error.contains("waste_circumference") && error.contains("waist_circumference"),
        "{error}"
    );
    assert_eq!(
        Measure::parse("bone_distance:upperarm01.L:wrist.L").unwrap(),
        Measure::BoneDistance("upperarm01.L".to_owned(), "wrist.L".to_owned())
    );
}

#[test]
fn it_fits_a_stature_and_changes_nothing_it_was_not_asked_about() {
    let start = adult(&[("gender", 0.3)]);
    let result = fit(
        human(),
        &start,
        &[Target::new(Measure::Stature, 1.90)],
        &FitOptions::default(),
    )
    .unwrap();
    assert!(result.converged, "{result:?}");
    let got = value(&result.params, Measure::Stature);
    assert!((got - 1.90).abs() < 0.01, "stature {got}");
    // The fixed parameters stay where they were: age and gender are not free by default.
    assert_eq!(result.params.phenotype["age"], 0.667);
    assert_eq!(result.params.phenotype["gender"], 0.3);
}

#[test]
fn it_recovers_a_whole_set_of_measurements_from_a_known_person() {
    // Measure a particular person, then ask for those numbers from the average one: the fit must find a body that has them.
    let person = adult(&[
        ("gender", 0.2),
        ("height", 0.8),
        ("weight", 0.7),
        ("muscle", 0.65),
    ]);
    let wanted = [
        Measure::Stature,
        Measure::Mass,
        Measure::WaistCircumference,
        Measure::HipCircumference,
        Measure::ChestCircumference,
    ];
    let targets: Vec<Target> = wanted
        .iter()
        .map(|m| Target::new(m.clone(), value(&person, m.clone())))
        .collect();
    let started = std::time::Instant::now();
    let result = fit(
        human(),
        &adult(&[("gender", 0.2)]),
        &targets,
        &FitOptions::default(),
    )
    .unwrap();
    // Measured: 1.5 s optimised, about 30 s in a debug build.
    let budget = if cfg!(debug_assertions) { 120.0 } else { 20.0 };
    assert!(
        started.elapsed().as_secs_f32() < budget,
        "fit took {:?}",
        started.elapsed()
    );
    for target in &targets {
        let got = value(&result.params, target.measure.clone());
        let tolerance = if target.measure == Measure::Mass {
            1.5
        } else {
            0.015
        };
        assert!(
            (got - target.value).abs() < tolerance,
            "{:?}: asked {} got {got}",
            target.measure,
            target.value
        );
    }
    // The report says the same thing the measurements do.
    assert_eq!(result.achieved.len(), targets.len());
    for achieved in &result.achieved {
        let direct = value(&result.params, achieved.measure.clone());
        assert!(
            (achieved.value - direct).abs() < 1e-4,
            "{achieved:?} against {direct}"
        );
    }
}

#[test]
fn an_impossible_request_gets_the_nearest_body_and_says_so() {
    let result = fit(
        human(),
        &adult(&[]),
        &[Target::new(Measure::Stature, 3.0)],
        &FitOptions::default(),
    )
    .unwrap();
    assert!(!result.converged);
    let got = value(&result.params, Measure::Stature);
    // Measured: the model's sliders and `measure-*` shapes together reach about 2.38 m, so "as tall as the model allows" is anywhere from 1.9 to 2.5 m.
    assert!(
        got > 1.85 && got < 2.5,
        "as tall as the model allows: {got}"
    );
    for (name, v) in &result.params.phenotype {
        assert!((0.0..=1.0).contains(v), "{name} stayed in range: {v}");
    }
    for (name, v) in &result.params.local {
        assert!((-1.0..=1.0).contains(v), "{name} stayed in range: {v}");
    }
}

#[test]
fn bone_distances_fit_limb_lengths() {
    let arm = Measure::BoneDistance("upperarm01.L".to_owned(), "wrist.L".to_owned());
    let before = value(&adult(&[]), arm.clone());
    let wanted = before + 0.04;
    let result = fit(
        human(),
        &adult(&[]),
        &[Target::new(arm.clone(), wanted)],
        &FitOptions::default(),
    )
    .unwrap();
    assert!((value(&result.params, arm) - wanted).abs() < 0.008);
}

#[test]
fn fitting_is_deterministic() {
    let targets = [
        Target::new(Measure::Stature, 1.8),
        Target::new(Measure::WaistCircumference, 0.9),
    ];
    let a = fit(human(), &adult(&[]), &targets, &FitOptions::default()).unwrap();
    let b = fit(human(), &adult(&[]), &targets, &FitOptions::default()).unwrap();
    assert_eq!(a.params, b.params);
}

#[test]
fn bad_requests_are_refused_naming_the_fix() {
    let none = fit(human(), &adult(&[]), &[], &FitOptions::default());
    assert!(format!("{:?}", none.unwrap_err()).contains("target"));
    let nonsense = fit(
        human(),
        &adult(&[]),
        &[Target::new(Measure::Stature, -1.0)],
        &FitOptions::default(),
    );
    assert!(nonsense.is_err());
    let unknown_free = FitOptions {
        free_phenotype: vec!["heigth".to_owned()],
        ..FitOptions::default()
    };
    let error = format!(
        "{:?}",
        fit(
            human(),
            &adult(&[]),
            &[Target::new(Measure::Stature, 1.8)],
            &unknown_free
        )
        .unwrap_err()
    );
    assert!(
        error.contains("heigth") && error.contains("height"),
        "{error}"
    );
}
