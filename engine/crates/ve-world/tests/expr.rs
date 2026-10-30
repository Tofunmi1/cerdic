//! The field-expression engine (ADR 0060 phase 1): grammar, types, the shared corpus, noise, diagnostics, caps, attributes, bound
//! fields and bit-reproducibility. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "fixtures unwrap valid programs and turn small counts into coordinates"
)]
use ve_world::expr::{
    Bindings, Diagnostic, Expr, MAX_OPERATIONS, MAX_SOURCE_BYTES, Sample, Type, Value,
};

const CORPUS: &str = include_str!("../../../tests/field-expr/corpus.txt");

fn compile(source: &str) -> Expr {
    Expr::compile(source, &Bindings::default())
        .unwrap_or_else(|d| unreachable!("`{source}` should compile: {d:?}"))
}

fn errors(source: &str) -> Vec<Diagnostic> {
    match Expr::compile(source, &Bindings::default()) {
        Err(found) => found,
        Ok(_) => unreachable!("`{source}` should be refused"),
    }
}

fn numbers(text: &str) -> Vec<f32> {
    text.split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().parse().unwrap())
        .collect()
}

fn corpus_sample(inputs: &str) -> Sample<'static> {
    let mut sample = Sample::default();
    for part in inputs.split_whitespace() {
        let (key, value) = part.split_once('=').unwrap();
        let v = numbers(value);
        match key {
            "p" => sample.p = [v[0], v[1], v[2]],
            "n" => sample.n = [v[0], v[1], v[2]],
            "uv" => sample.uv = [v[0], v[1]],
            other => unreachable!("unknown corpus input {other}"),
        }
    }
    sample
}

#[test]
fn every_line_of_the_shared_corpus_is_reproduced() {
    let mut cases = 0;
    for line in CORPUS
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let parts: Vec<&str> = line.splitn(3, "##").collect();
        assert_eq!(parts.len(), 3, "corpus line has three parts: {line}");
        let expr = compile(parts[0].trim());
        let got = expr.eval(&corpus_sample(parts[1])).floats();
        let want = numbers(parts[2]);
        assert_eq!(
            got.len(),
            want.len(),
            "`{line}` has {} components, got {got:?}",
            want.len()
        );
        for (g, w) in got.iter().zip(&want) {
            assert!(
                (g - w).abs() <= 1e-4 * w.abs().max(1.0),
                "`{line}`: got {got:?}, want {want:?}"
            );
        }
        cases += 1;
    }
    assert!(
        cases >= 60,
        "the corpus holds at least 60 cases, found {cases}"
    );
}

#[test]
fn output_types_are_reported_and_can_be_required() {
    assert_eq!(compile("out = 1.0").output_type(), Type::Float);
    assert_eq!(compile("out = p.xy").output_type(), Type::Vec2);
    assert_eq!(compile("out = p").output_type(), Type::Vec3);
    assert_eq!(compile("out = vec4(1.0)").output_type(), Type::Vec4);
    assert_eq!(compile("out = p.x > 0.0").output_type(), Type::Bool);
    let wanted = Expr::compile_as("out = p", &Bindings::default(), Type::Float);
    let Err(found) = wanted else {
        unreachable!("a vec3 is not a float")
    };
    assert!(
        found[0].message.to_lowercase().contains("float"),
        "{found:?}"
    );
    assert!(Expr::compile_as("out = p.x * 2.0", &Bindings::default(), Type::Float).is_ok());
}

#[test]
fn bound_fields_and_attributes_are_values_the_host_supplies() {
    let bindings = Bindings {
        fields: vec![
            ("wet".to_owned(), Type::Float),
            ("tint".to_owned(), Type::Vec3),
        ],
    };
    let expr = Expr::compile("out = wet * 2.0 + tint.y", &bindings).unwrap();
    let bound = [Value::Float(0.3), Value::Vec3([0.0, 0.5, 0.0])];
    let sample = Sample {
        bound: &bound,
        ..Sample::default()
    };
    assert!((expr.eval(&sample).floats()[0] - 1.1).abs() < 1e-5);
    assert!(
        Expr::compile("out = dry", &bindings).is_err(),
        "an unbound name is refused"
    );

    let expr = compile("out = attr(\"age\") * 0.5 + attr3(\"tint\").x");
    assert_eq!(
        expr.attributes(),
        &[
            ("age".to_owned(), Type::Float),
            ("tint".to_owned(), Type::Vec3)
        ],
        "the host learns which columns it must supply"
    );
    let attrs = [
        ("age", Value::Float(4.0)),
        ("tint", Value::Vec3([0.25, 0.0, 0.0])),
    ];
    let sample = Sample {
        attrs: &attrs,
        ..Sample::default()
    };
    assert!((expr.eval(&sample).floats()[0] - 2.25).abs() < 1e-5);
    assert!(
        expr.eval(&Sample::default()).floats()[0].abs() < 1e-9,
        "a missing attribute reads as zero"
    );
}

fn position(i: usize) -> [f32; 3] {
    // A deterministic scatter of points in a 40 m cube, from a small integer hash.
    let h = |k: u32| -> f32 {
        let mut x = (i as u32)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(k.wrapping_mul(0x9E37_79B9));
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x & 0xFFFF) as f32 / 65_535.0 * 40.0 - 20.0
    };
    [h(1), h(2), h(3)]
}

fn sample_stats(source: &str, count: usize) -> (f32, f32, f32, f32) {
    let expr = compile(source);
    let values: Vec<f32> = (0..count)
        .map(|i| {
            expr.eval(&Sample {
                p: position(i),
                ..Sample::default()
            })
            .floats()[0]
        })
        .collect();
    let mean = values.iter().sum::<f32>() / count as f32;
    let var = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / count as f32;
    (
        values.iter().copied().fold(f32::MAX, f32::min),
        values.iter().copied().fold(f32::MIN, f32::max),
        mean,
        var.sqrt(),
    )
}

#[test]
fn perlin_noise_is_bounded_zero_on_the_lattice_continuous_and_seeded() {
    let (lo, hi, mean, spread) = sample_stats("out = perlin(p, 1.0)", 4000);
    assert!(lo >= -1.0001 && hi <= 1.0001, "range {lo}..{hi}");
    assert!(
        lo < -0.5 && hi > 0.5,
        "it uses most of its range: {lo}..{hi}"
    );
    assert!(
        mean.abs() < 0.05 && (0.15..0.6).contains(&spread),
        "mean {mean}, spread {spread}"
    );
    let expr = compile("out = perlin(p, 1.0)");
    assert!(
        expr.eval(&Sample {
            p: [3.0, -2.0, 7.0],
            ..Sample::default()
        })
        .floats()[0]
            .abs()
            < 1e-5,
        "gradient noise is zero on lattice points"
    );
    let mut moved = 0;
    for i in 0..2000 {
        let a = expr
            .eval(&Sample {
                p: position(i),
                ..Sample::default()
            })
            .floats()[0];
        let p = position(i);
        let b = expr
            .eval(&Sample {
                p: [p[0] + 0.001, p[1], p[2]],
                ..Sample::default()
            })
            .floats()[0];
        assert!((a - b).abs() < 0.01, "continuous: {a} then {b}");
        let other = compile("out = perlin(p, 2.0)")
            .eval(&Sample {
                p,
                ..Sample::default()
            })
            .floats()[0];
        moved += usize::from((a - other).abs() > 1e-3);
    }
    assert!(
        moved > 1800,
        "another seed is another field ({moved} of 2000 differ)"
    );
}

#[test]
fn fbm_adds_detail_with_octaves_and_stays_in_range_and_ridged_and_voronoi_have_their_ranges() {
    let rough = |octaves: &str| -> f32 {
        let expr = compile(&format!("out = fbm(p, {octaves}, 1.0)"));
        (0..1500)
            .map(|i| {
                let p = position(i);
                let a = expr
                    .eval(&Sample {
                        p,
                        ..Sample::default()
                    })
                    .floats()[0];
                let b = expr
                    .eval(&Sample {
                        p: [p[0] + 0.02, p[1], p[2]],
                        ..Sample::default()
                    })
                    .floats()[0];
                (a - b).abs()
            })
            .sum::<f32>()
            / 1500.0
    };
    // With gain 0.5 and lacunarity 2 every octave adds the same slope, and the sum is divided by the amplitudes (about 2 for six
    // octaves), so six octaves are only about 10 to 25 percent rougher at this scale than one: a clear, measurable increase.
    let (one, six) = (rough("1.0"), rough("6.0"));
    assert!(
        six > one * 1.05,
        "more octaves give finer detail: {one} against {six}"
    );
    let (lo, hi, _, _) = sample_stats("out = fbm(p, 5.0, 3.0)", 3000);
    assert!(
        lo >= -1.0001 && hi <= 1.0001,
        "fbm is normalised into -1..1: {lo}..{hi}"
    );
    let (lo, hi, _, _) = sample_stats("out = ridged(p, 4.0, 2.0)", 3000);
    assert!(
        lo >= -0.0001 && hi <= 1.0001 && hi > 0.7,
        "ridged noise is in 0..1: {lo}..{hi}"
    );
    let (lo, hi, mean, _) = sample_stats("out = voronoi(p, 5.0)", 3000);
    assert!(
        (0.0..0.05).contains(&lo) && hi < 1.6,
        "voronoi is a distance to the nearest feature point: {lo}..{hi}"
    );
    assert!((0.35..0.8).contains(&mean), "mean cell distance {mean}");
}

#[test]
fn evaluation_is_reproducible_and_batching_matches_single_samples() {
    let expr = compile(
        "w = fbm(p * 0.3, 4.0, 7.0); out = mix(w, ridged(p, 3.0, 1.0), smoothstep(-5.0, 5.0, p.y))",
    );
    let samples: Vec<Sample> = (0..500)
        .map(|i| Sample {
            p: position(i),
            ..Sample::default()
        })
        .collect();
    let single: Vec<u32> = samples
        .iter()
        .map(|s| expr.eval(s).floats()[0].to_bits())
        .collect();
    let again: Vec<u32> = samples
        .iter()
        .rev()
        .map(|s| expr.eval(s).floats()[0].to_bits())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    assert_eq!(single, again, "order does not matter, nothing is stateful");
    let mut batch = Vec::new();
    expr.eval_floats(&samples, &mut batch).unwrap();
    assert_eq!(
        batch.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        single,
        "batch and single agree to the bit"
    );
    assert!(
        compile("out = p")
            .eval_floats(&samples, &mut batch)
            .is_err(),
        "a vec3 expression cannot fill a float batch"
    );
}

#[test]
fn results_are_always_finite() {
    for source in [
        "out = 1.0 / 0.0",
        "out = sqrt(-4.0)",
        "out = log(-1.0)",
        "out = pow(0.0, -1.0)",
        "out = exp(1000.0)",
        "out = pow(10.0, 100.0)",
        "out = atan2(0.0, 0.0)",
        "out = normalize(vec3(0.0))",
        "out = 5.0 % 0.0",
    ] {
        let v = compile(source).eval(&Sample::default()).floats();
        assert!(v.iter().all(|x| x.is_finite()), "`{source}` gave {v:?}");
    }
}

#[test]
fn the_engine_uses_no_platform_dependent_math_functions() {
    // std's sin, cos, exp, ln, powf and friends call the platform libm, which differs between native and WASM.
    let banned = [
        ".sin(",
        ".cos(",
        ".tan(",
        ".asin(",
        ".acos(",
        ".atan(",
        ".atan2(",
        ".exp(",
        ".exp2(",
        ".ln(",
        ".log(",
        ".log2(",
        ".log10(",
        ".powf(",
        ".powi(",
        ".sinh(",
        ".cosh(",
        ".tanh(",
        ".cbrt(",
        ".hypot(",
        ".mul_add(",
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut scanned = 0;
    let mut stack = vec![root.join("expr.rs"), root.join("expr")];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            stack.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            for word in banned {
                assert!(!text.contains(word), "{} uses {word}", path.display());
            }
            scanned += 1;
        }
    }
    assert!(
        scanned >= 2,
        "the expression module is split into files, found {scanned}"
    );
}

#[test]
fn mistakes_get_located_diagnostics_that_say_how_to_fix_them() {
    let missing = errors("a = 1.0");
    assert!(missing[0].message.contains("out"), "{missing:?}");
    let unknown = errors("a = 1.0\nb = nope\nout = a");
    assert_eq!(
        (unknown[0].line, unknown[0].column),
        (2, 5),
        "points at `nope`: {unknown:?}"
    );
    assert!(unknown[0].message.contains("nope") && !unknown[0].suggestion.is_empty());
    let typo = errors("out = sinn(1.0)");
    assert!(
        typo[0].suggestion.contains("sin"),
        "a near miss names the function: {typo:?}"
    );
    let arity = errors("out = mix(1.0, 2.0)");
    assert!(
        arity[0].message.contains("mix") && arity[0].message.contains('3'),
        "{arity:?}"
    );
    let mixed = errors("out = vec2(1.0, 2.0) + vec3(1.0, 2.0, 3.0)");
    assert!(
        mixed[0].message.to_lowercase().contains("vec2")
            && mixed[0].message.to_lowercase().contains("vec3"),
        "{mixed:?}"
    );
    assert!(
        !errors("p = vec3(0.0); out = 1.0")[0].message.is_empty(),
        "inputs cannot be assigned"
    );
    assert!(
        errors("out = later; later = 1.0")[0]
            .message
            .contains("later"),
        "no use before definition"
    );
    assert!(
        !errors("out = select(1.0, 2.0, 3.0)")[0].message.is_empty(),
        "select needs a bool first"
    );
    assert!(
        !errors("out = p.w")[0].message.is_empty(),
        "a vec3 has no w"
    );
    assert!(
        !errors("out = 1.0 +")[0].message.is_empty(),
        "syntax errors are located too"
    );
    for d in unknown.iter().chain(&typo).chain(&arity) {
        assert!(d.line >= 1 && d.column >= 1);
    }
}

#[test]
fn programs_are_bounded_in_size_and_operations() {
    let long = format!("out = 1.0{}", " ".repeat(MAX_SOURCE_BYTES));
    assert!(
        errors(&long)[0].message.to_lowercase().contains("long")
            || errors(&long)[0].message.contains("8192")
    );
    let busy = format!("out = {}", vec!["p.x"; MAX_OPERATIONS + 20].join(" + "));
    let found = errors(&busy);
    assert!(
        found[0].message.to_lowercase().contains("complex")
            || found[0].message.contains("operations"),
        "{found:?}"
    );
    let ok = format!("out = {}", vec!["p.x"; 60].join(" + "));
    assert!(
        Expr::compile(&ok, &Bindings::default())
            .unwrap()
            .operation_count()
            <= MAX_OPERATIONS
    );
}

#[test]
#[ignore = "timing; run with --release -- --ignored"]
fn a_million_noise_samples_take_well_under_a_few_seconds() {
    let expr = compile("out = fbm(p * 0.1, 4.0, 1.0)");
    let samples: Vec<Sample> = (0..1_000_000)
        .map(|i| Sample {
            p: position(i % 4096),
            ..Sample::default()
        })
        .collect();
    let mut out = Vec::new();
    let start = std::time::Instant::now();
    expr.eval_floats(&samples, &mut out).unwrap();
    let seconds = start.elapsed().as_secs_f32();
    println!("1,000,000 fbm samples in {seconds:.2} s");
    assert!(seconds < 4.0, "{seconds} s");
}
