//! `solid.style`: one word sets shading, outline and texture finishing, and `realistic` takes an earlier style away.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_style").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| e.message)
}

fn styled(style: &str, extra: &str) -> ve_modeling::Material {
    let text = format!(
        "box = solid.box@1(size=[1, 1, 1])\n\
         paint = solid.material@1(box, color=[0.7, 0.4, 0.3], roughness=0.5)\n\
         look = solid.style@1(paint, style=\"{style}\"{extra})\n\
         output thing = look\n"
    );
    evaluate(&text).unwrap().parts[0].material.clone().unwrap()
}

#[test]
fn each_style_sets_its_shading() {
    let anime = styled("anime", "");
    let toon = anime.toon.unwrap();
    assert_eq!(toon.bands, 2);
    assert!(toon.outline > 1.0 && toon.rim > 0.0);
    assert!(anime.texture_style.is_none());

    let painterly = styled("painterly", "");
    assert_eq!(painterly.texture_style.as_deref(), Some("painterly"));
    assert!(
        painterly.toon.as_ref().unwrap().feather > 0.2,
        "soft band edges"
    );
    assert!(
        painterly.toon.unwrap().outline.abs() < 1e-6,
        "painted looks have no outline"
    );

    assert_eq!(styled("retro", "").texture_style.as_deref(), Some("retro"));
    let ink = styled("ink", "");
    assert_eq!(ink.texture_style.as_deref(), Some("ink"));
    assert!(
        ink.toon.unwrap().outline >= 3.0 - 1e-6,
        "ink has a thick line"
    );
    let flat = styled("flat", "").toon.unwrap();
    assert_eq!(flat.bands, 1);
    assert_eq!(flat.shade_color, [1.0, 1.0, 1.0], "flat has no shadow tint");
}

#[test]
fn colours_and_roughness_are_kept() {
    let material = styled("painterly", "");
    assert_eq!(material.color, [0.7, 0.4, 0.3]);
    assert!((material.roughness - 0.5).abs() < 1e-6);
}

#[test]
fn realistic_takes_an_earlier_style_away() {
    let text = "box = solid.box@1(size=[1, 1, 1])\n\
        paint = solid.material@1(box, color=[0.7, 0.4, 0.3], roughness=0.5)\n\
        loud = solid.style@1(paint, style=\"ink\")\n\
        calm = solid.style@1(loud, style=\"realistic\")\n\
        output thing = calm\n";
    let material = evaluate(text).unwrap().parts[0].material.clone().unwrap();
    assert!(material.toon.is_none() && material.texture_style.is_none());
}

#[test]
fn outline_scale_multiplies_the_outline() {
    let double = styled("anime", ", outline_scale=2").toon.unwrap().outline;
    let none = styled("anime", ", outline_scale=0").toon.unwrap().outline;
    let normal = styled("anime", "").toon.unwrap().outline;
    assert!((double - 2.0 * normal).abs() < 1e-5);
    assert!(none.abs() < 1e-6);
}

#[test]
fn an_unknown_style_is_refused_with_the_choices() {
    let text = "box = solid.box@1(size=[1, 1, 1])\nlook = solid.style@1(box, style=\"watercolour\")\noutput thing = look\n";
    let batch = parse_text(&builtin_registry(), text);
    let message = match batch {
        Err(error) => format!("{error:?}"),
        Ok(batch) => {
            let mut session = Session::with_builtin(2);
            let outcome = session.apply(&CommandBatch {
                doc: session.document().id.clone(),
                base_revision: 0,
                transaction: TransactionId::parse("t_g11_style_bad").unwrap(),
                label: None,
                commands: batch.commands,
            });
            format!("{outcome:?}")
        }
    };
    assert!(
        message.contains("anime") || message.contains("not one of"),
        "{message}"
    );
}
