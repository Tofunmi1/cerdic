//! Consistent numeric budget diagnostics for preflight and actual output checks.
use crate::Fail;

/// The most memory one document's geometry and storage may need: the procedural character demo needs 275 MB, so the old 256 MiB refused it.
pub(crate) const MEMORY_LIMIT: usize = 1024 * 1024 * 1024;

#[expect(
    clippy::cast_precision_loss,
    reason = "diagnostic estimates may round above the bounded budgets; comparisons use usize"
)]
pub(crate) fn exceeded(code: &str, limit: usize, observed: usize) -> Fail {
    exceeded_with(code, limit, observed, "")
}

/// Like [`exceeded`], with a sentence of detail added to the suggestion (which parts are the heaviest, for example).
#[expect(
    clippy::cast_precision_loss,
    reason = "diagnostic estimates may round above the bounded budgets; comparisons use usize"
)]
pub(crate) fn exceeded_with(code: &str, limit: usize, observed: usize, detail: &str) -> Fail {
    let message = format!(
        "{} budget exceeded: {} against a limit of {}",
        code.strip_prefix("budget.")
            .unwrap_or(code)
            .replace('_', " "),
        grouped(observed),
        grouped(limit)
    );
    let diagnostic =
        ve_graph::Diagnostic::error(code, message).with_limit(limit as f64, observed as f64);
    let diagnostic = match suggestion(code, limit, observed) {
        Some(text) if detail.is_empty() => diagnostic.suggest(text),
        Some(text) => diagnostic.suggest(format!("{text}. {detail}")),
        None => diagnostic,
    };
    Box::new(diagnostic)
}

/// A count with thousands separators: `1071992` as `1,071,992`.
pub(crate) fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The parts with the most triangles, biggest first, as a sentence a person (or a model) can act on: which to make lighter first.
/// `counts` is every part's name and triangle count; empty when nothing is known.
pub(crate) fn heaviest_text(counts: &[(String, usize)], show: usize) -> String {
    if counts.is_empty() {
        return String::new();
    }
    let total: usize = counts.iter().map(|(_, triangles)| triangles).sum();
    let mut sorted: Vec<&(String, usize)> = counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let listed: Vec<String> = sorted
        .iter()
        .take(show)
        .map(|(name, triangles)| format!("{name} {}", grouped(*triangles)))
        .collect();
    let shown: usize = sorted
        .iter()
        .take(show)
        .map(|(_, triangles)| triangles)
        .sum();
    let share = if total == 0 { 0 } else { shown * 100 / total };
    format!(
        "The heaviest parts: {} ({share}% of {} triangles in {} parts); lowering those first helps most: fewer columns, rows or segments, a larger edge_length on field.to_solid, less subdivision, or fewer copies",
        listed.join(", "),
        grouped(total),
        counts.len()
    )
}

/// Every part's name and triangle count, for [`heaviest_text`].
pub(crate) fn part_counts(parts: &[crate::Part]) -> Vec<(String, usize)> {
    parts
        .iter()
        .map(|part| (part.name.clone(), part.indices.len() / 3))
        .collect()
}

/// How many copies of a part can be drawn at once. Without a distance band that is every copy. A set drawn only between two camera
/// distances (`scatter.place` show_from / show_to) can be seen from anywhere, so the most copies it can show is the largest number of
/// copies that lie within the band of one camera standing among the copies (a camera is tried at up to `CAMERAS` evenly spaced copies).
pub(crate) fn drawn_copies(part: &crate::Part) -> usize {
    const CAMERAS: usize = 256;
    let Some(transforms) = part.transforms.as_ref() else {
        return 1;
    };
    let Some((from, to)) = part.visible_range else {
        return transforms.len();
    };
    let position = |m: &[f32; 16]| [m[12], m[13], m[14]];
    let step = transforms.len().div_ceil(CAMERAS).max(1);
    transforms
        .iter()
        .step_by(step)
        .map(|camera| {
            let at = position(camera);
            transforms
                .iter()
                .filter(|other| {
                    let p = position(other);
                    let distance =
                        ((p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2) + (p[2] - at[2]).powi(2))
                            .sqrt();
                    distance >= from && distance <= to
                })
                .count()
        })
        .max()
        .unwrap_or(0)
}

/// Triangle accounting over the parts of a result: what is stored (a shared mesh once, however many placements use it, so
/// memory and work follow it) and what would be drawn at full detail (every placement counted).
#[derive(Default)]
pub(crate) struct Triangles {
    stored: usize,
    drawn: usize,
    /// The output that draws the most, and how much, for the heavy-scene warning.
    heaviest: Option<(String, usize)>,
    /// Every part counted so far, for naming the heaviest ones when a budget is exceeded.
    counts: Vec<(String, usize)>,
}

impl Triangles {
    /// Counts the parts one output made.
    ///
    /// # Errors
    /// `budget.triangles` when the stored geometry is over the quality's limit, `budget.drawn_triangles` when what it would
    /// draw is over the (much larger) drawn limit.
    pub(crate) fn add(
        &mut self,
        node: &ve_graph::NodeId,
        parts: &[crate::Part],
        quality: crate::Quality,
    ) -> Result<(), Fail> {
        let mut drawn_here = 0_usize;
        for part in parts {
            let own = part.indices.len() / 3;
            self.counts.push((part.name.clone(), own));
            self.stored = self.stored.saturating_add(own);
            drawn_here = drawn_here.saturating_add(own.saturating_mul(drawn_copies(part)));
        }
        self.drawn = self.drawn.saturating_add(drawn_here);
        if self
            .heaviest
            .as_ref()
            .is_none_or(|(_, most)| drawn_here > *most)
        {
            self.heaviest = Some((node.to_string(), drawn_here));
        }
        if self.stored > quality.triangles() {
            return Err(exceeded_with(
                "budget.triangles",
                quality.triangles(),
                self.stored,
                &heaviest_text(&self.counts, 5),
            ));
        }
        if self.drawn > quality.drawn_triangles() {
            return Err(exceeded_with(
                "budget.drawn_triangles",
                quality.drawn_triangles(),
                self.drawn,
                &heaviest_text(&self.counts, 5),
            ));
        }
        Ok(())
    }

    /// A warning when the scene would draw more than [`crate::result::DRAWN_TRIANGLES_WARNING`] triangles at full detail, on
    /// the output that draws the most.
    pub(crate) fn heavy_warning(&self) -> Option<crate::EvaluationWarning> {
        let (node, _) = self.heaviest.as_ref()?;
        (self.drawn > crate::result::DRAWN_TRIANGLES_WARNING).then(|| crate::EvaluationWarning {
            code: "budget.drawn_heavy".to_owned(),
            node: node.clone(),
            related: Vec::new(),
            message: format!(
                "the scene draws about {} triangles at full detail ({} stored): a viewer may struggle; use lighter placed shapes or fewer placements",
                self.drawn, self.stored
            ),
        })
    }
}

/// What a caller can change so a numeric budget is not exceeded again, by code. The real limit and
/// observed amount are written into the text where the code has them.
fn suggestion(code: &str, limit: usize, observed: usize) -> Option<String> {
    Some(match code {
        "budget.triangles" => format!(
            "the result would need about {observed} triangles, over the {limit} budget: lower the mesh detail (segments, subdivision levels or sections), or ask for a coarser quality (placed copies of one shape count once)"
        ),
        "budget.drawn_triangles" => format!(
            "the placements would draw about {observed} triangles at full detail, over the {limit} limit: use a lighter placed shape or fewer placements"
        ),
        "budget.memory" => format!(
            "the result would need about {observed} bytes, over the {limit} budget: decimate or lower the detail, or close another document"
        ),
        "budget.instances" => format!(
            "the placements would number {observed}, over the {limit} budget: lower count_x, count_y, count_z or count"
        ),
        "budget.sockets" => format!(
            "the solid would carry {observed} sockets, over the {limit} budget: remove the sockets this part does not need"
        ),
        "budget.lights" => format!(
            "the scene has {observed} lights, over the {limit} budget: remove emitters or share one"
        ),
        "budget.field_cells" => format!(
            "the field grid needs {observed} cells, over the {limit} budget: raise edge_length, or shrink the min and max bounds"
        ),
        "budget.field_ops" => format!(
            "the field needs {observed} work units, over the {limit} budget: raise edge_length, or use fewer field primitives"
        ),
        "budget.field_tape" => format!(
            "the field needs {observed} instructions, over the {limit} budget: use fewer or simpler field primitives"
        ),
        "budget.ao_triangles" => format!(
            "ambient occlusion needs {observed} triangles, over the {limit} budget: lower the mesh detail"
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::drawn_copies;
    use crate::Part;

    /// A part with one copy at each x in `xs` (metres, along a line) and the given band.
    fn row(xs: &[f32], range: Option<(f32, f32)>) -> Part {
        let transforms = xs
            .iter()
            .map(|x| {
                let mut m = [0.0; 16];
                m[0] = 1.0;
                m[5] = 1.0;
                m[10] = 1.0;
                m[15] = 1.0;
                m[12] = *x;
                m
            })
            .collect();
        Part {
            node: ve_graph::NodeId::parse("n_01234567").expect("fixture id"),
            name: "row".to_owned(),
            positions: Vec::new(),
            indices: Vec::new(),
            uvs: Vec::new(),
            occlusion: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            volume: 0.0,
            material: None,
            frames: std::collections::BTreeMap::new(),
            animation: Vec::new(),
            state_machine: None,
            behavior: None,
            parent: None,
            bone: None,
            skin: None,
            morph_targets: Vec::new(),
            ik: None,
            fx: None,
            ragdoll: None,
            cloth: None,
            fluid: None,
            sim: None,
            body: None,
            transforms: Some(transforms),
            visible_range: range,
            visible_fade: 0.0,
            instance_indices: Vec::new(),
            region: 0,
            baked: None,
        }
    }

    #[test]
    fn a_set_without_a_band_draws_every_copy() {
        let xs: Vec<f32> = (0..100).map(|i| i as f32 * 10.0).collect();
        assert_eq!(drawn_copies(&row(&xs, None)), 100);
    }

    #[test]
    fn a_banded_set_counts_the_copies_one_camera_can_reach() {
        // Copies every 10 m along 1 km; a camera among them reaches those 20 to 45 m away: 2 on each side at 20, 30, 40.
        let xs: Vec<f32> = (0..100).map(|i| i as f32 * 10.0).collect();
        assert_eq!(drawn_copies(&row(&xs, Some((20.0, 45.0)))), 6);
        // A band with no far limit from 140 m: from a camera at one end, the 86 copies beyond 140 m.
        assert_eq!(drawn_copies(&row(&xs, Some((140.0, f32::INFINITY)))), 86);
    }

    #[test]
    fn counts_are_written_with_separators() {
        assert_eq!(super::grouped(0), "0");
        assert_eq!(super::grouped(999), "999");
        assert_eq!(super::grouped(1_000), "1,000");
        assert_eq!(super::grouped(1_071_992), "1,071,992");
    }

    #[test]
    fn the_heaviest_parts_are_named_biggest_first_with_their_share() {
        let counts = vec![
            ("pillars".to_owned(), 3_584),
            ("peak_h".to_owned(), 16_800),
            ("bull".to_owned(), 12_880),
            ("tiny".to_owned(), 10),
        ];
        let text = super::heaviest_text(&counts, 2);
        assert!(text.contains("peak_h 16,800, bull 12,880"), "{text}");
        assert!(
            !text.contains("pillars"),
            "only the two heaviest are listed: {text}"
        );
        assert!(text.contains("of 33,274 triangles in 4 parts"), "{text}");
        assert!(
            text.contains("edge_length"),
            "it says what to change: {text}"
        );
        assert_eq!(super::heaviest_text(&[], 5), "");
    }

    #[test]
    fn an_exceeded_budget_says_how_far_over_and_names_the_heaviest_parts() {
        let counts = vec![("peak_h".to_owned(), 16_800), ("bull".to_owned(), 12_880)];
        let error = super::exceeded_with(
            "budget.ao_triangles",
            1_000_000,
            1_071_992,
            &super::heaviest_text(&counts, 5),
        );
        assert_eq!(error.code, "budget.ao_triangles");
        assert_eq!(
            error.message,
            "ao triangles budget exceeded: 1,071,992 against a limit of 1,000,000"
        );
        let suggestion = error.suggestion.clone().unwrap_or_default();
        assert!(suggestion.contains("peak_h 16,800"), "{suggestion}");
        assert!(
            suggestion.contains("over the 1000000 budget"),
            "{suggestion}"
        );
        // without detail the old suggestion stands alone
        let plain = super::exceeded("budget.triangles", 10, 20);
        assert!(
            !plain
                .suggestion
                .clone()
                .unwrap_or_default()
                .contains("heaviest")
        );
    }
}
