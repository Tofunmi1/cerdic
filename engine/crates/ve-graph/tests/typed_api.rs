//! G3: the typed API declarations are generated from the operator registry: every ready operator once, with its kinds, parameters, choices and requirements, deterministic, and the checked-in file is current.
#![expect(clippy::unwrap_used, reason = "fixtures")]
use ve_graph::{builtin_registry, operators_d_ts};

#[test]
fn every_ready_operator_is_declared_exactly_once() {
    let registry = builtin_registry();
    let text = operators_d_ts(&registry);
    let ready: Vec<String> = registry.ready().map(|s| s.id.to_string()).collect();
    assert!(ready.len() > 200, "{} ready operators", ready.len());
    // Each operator has a params interface named from its id, declared once.
    for id in &ready {
        let (name, version) = id.split_once('@').unwrap();
        let pascal: String = name
            .split(['.', '_'])
            .map(|s| {
                let mut c = s.chars();
                c.next().map_or(String::new(), |f| {
                    f.to_uppercase().collect::<String>() + c.as_str()
                })
            })
            .collect();
        let interface = format!("export interface {pascal}V{version}Params {{");
        assert_eq!(
            text.matches(&interface).count(),
            1,
            "{id} is declared once ({interface})"
        );
    }
    assert_eq!(
        text.matches("Params {").count(),
        ready.len(),
        "no extra interfaces"
    );
    // And every id is in the OperatorId union, so a pack validator can know what the engine has.
    for id in &ready {
        assert_eq!(
            text.matches(&format!("'{id}'")).count(),
            1,
            "{id} is in OperatorId once"
        );
    }
}

#[test]
fn kinds_parameters_choices_and_requirements_come_from_the_specs() {
    let text = operators_d_ts(&builtin_registry());
    // A choice parameter is a union of its words.
    assert!(
        text.contains("element?: 'face' | 'edge' | 'vertex' | 'loop';"),
        "poly.select's element"
    );
    // A required parameter (no default) is not optional; a vector is a tuple.
    assert!(
        text.contains("  head: Vec3;") && text.contains("  tail: Vec3;"),
        "rig.bone needs head and tail"
    );
    // A number parameter also accepts a Number node.
    assert!(
        text.contains("radius?: number | Node<'Number'>;"),
        "solid.sphere radius"
    );
    // An operator that returns what it is given keeps the kind generic; one that makes a fixed kind says so.
    assert!(text.contains("move<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: XformMoveV1Params): T;"), "xform.move keeps its input's kind");
    assert!(
        text.contains("box(params: SolidBoxV1Params): Node<'Solid'>;"),
        "solid.box makes a solid and its size is required"
    );
    assert!(text.contains("simplify<T extends Node<'Solid' | 'Poly' | 'Inst'>>(input: T, params?: AnimSimplifyV1Params): T;"), "all-default parameters are optional");
    // Several inputs are an object of ports, an optional port is optional.
    assert!(text.contains("scatter: {"), "the scatter family");
    assert!(text.contains("place(inputs: { in: Node<'Solid' | 'Inst' | 'Poly'>; surface: Node<'Poly'>; where?: Node<'Pattern'> }"), "scatter.place's ports");
    // The operators added in this phase are there.
    for new in ["fracture(", "biomes(", "retarget(", "sim("] {
        assert!(text.contains(new), "{new}");
    }
    // Documentation travels with the declarations, kept to a sentence or two.
    assert!(
        text.contains("/** Breaks a closed shape into pieces along a random Voronoi pattern"),
        "poly.fracture's doc"
    );
    assert!(!text.contains("*/ */"), "comments are well formed");
}

#[test]
fn the_output_is_deterministic_and_the_checked_in_file_is_current() {
    let registry = builtin_registry();
    assert_eq!(
        operators_d_ts(&registry),
        operators_d_ts(&builtin_registry())
    );
    let checked_in = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../src/engine/operators.d.ts"
    ))
    .unwrap();
    assert_eq!(
        checked_in,
        operators_d_ts(&registry),
        "src/engine/operators.d.ts is stale: run bun run schema:generate"
    );
}
