//! B7: seeded scatter and jitter are deterministic, bounded, spaced and refuse instead of shrinking.
#![expect(
    clippy::unwrap_used,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn run(source: &str) -> Result<ve_modeling::Evaluation, String> {
    let parsed = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(907);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_scatter").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn transforms(source: &str) -> Vec<[f32; 16]> {
    run(source).unwrap().parts[0].transforms.clone().unwrap()
}

fn scatter(seed: u32, extra: &str) -> String {
    format!(
        "rock = solid.box@1(size=[0.1, 0.1, 0.1])\n\
         field = inst.scatter@1(rock, count=60, width=4, depth=3, seed={seed}{extra})\n\
         output field"
    )
}

#[test]
fn same_seed_gives_identical_placements_and_a_new_seed_changes_them() {
    let first = transforms(&scatter(7, ""));
    assert_eq!(first.len(), 60);
    assert_eq!(first, transforms(&scatter(7, "")));
    assert_ne!(first, transforms(&scatter(8, "")));
}

#[test]
fn placements_stay_inside_the_area_and_respect_min_spacing() {
    let spaced = transforms(&scatter(3, ", min_spacing=0.3"));
    assert_eq!(spaced.len(), 60);
    for matrix in &spaced {
        assert!(matrix[12].abs() <= 2.0 && matrix[14].abs() <= 1.5);
        assert!(matrix[13].abs() < 1.0e-6);
    }
    for (index, a) in spaced.iter().enumerate() {
        for b in &spaced[index + 1..] {
            let distance = ((a[12] - b[12]).powi(2) + (a[14] - b[14]).powi(2)).sqrt();
            assert!(distance >= 0.3 - 1.0e-4, "centres {distance} apart");
        }
    }
}

#[test]
fn unsatisfiable_spacing_refuses_with_a_typed_code() {
    let error = run(&scatter(1, ", min_spacing=2")).unwrap_err();
    assert!(error.contains("scatter.unsatisfiable"), "{error}");
}

#[test]
fn counts_over_the_cap_are_refused_by_the_operator_contract() {
    let source = "rock = solid.box@1(size=[1, 1, 1])\n\
                  field = inst.scatter@1(rock, count=20000, width=1, depth=1)\n\
                  output field";
    assert!(run(source).is_err());
}

#[test]
fn stacked_sets_over_the_instance_budget_refuse_before_any_placement_is_drawn() {
    let source = "rock = solid.box@1(size=[1, 1, 1])\n\
                  field = inst.scatter@1(rock, count=16384, width=500, depth=500)\n\
                  twice = inst.array@1(field, count_x=2, spacing=[1, 0, 0])\n\
                  output twice";
    let error = run(source).unwrap_err();
    assert!(error.contains("budget.instances"), "{error}");
}

#[test]
fn the_largest_scatter_is_fast_and_exact() {
    let started = std::time::Instant::now();
    let placed = transforms(
        "rock = solid.box@1(size=[0.1, 0.1, 0.1])\n\
         field = inst.scatter@1(rock, count=16384, width=200, depth=200, min_spacing=0.5, seed=2)\n\
         output field",
    );
    assert_eq!(placed.len(), 16_384);
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
}

#[test]
fn jitter_varies_in_place_and_each_element_depends_only_on_its_index() {
    let program = |count: u32| {
        format!(
            "post = solid.box@1(size=[0.1, 1, 0.1])\n\
             row = inst.array@1(post, count_x={count}, spacing=[1, 0, 0])\n\
             varied = inst.jitter@1(row, shift=[0.2, 0, 0.2], yaw_jitter=20deg, scale_jitter=0.2, seed=5)\n\
             output varied"
        )
    };
    let short = transforms(&program(4));
    let long = transforms(&program(9));
    assert_eq!((short.len(), long.len()), (4, 9));
    assert_eq!(
        short[..],
        long[..4],
        "adding elements must not move earlier ones"
    );
    let plain = transforms(&program(4).replace("shift=[0.2, 0, 0.2]", "shift=[0, 0, 0]"));
    assert_ne!(short, plain);
}
