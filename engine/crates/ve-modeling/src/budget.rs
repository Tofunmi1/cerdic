//! Consistent numeric budget diagnostics for preflight and actual output checks.
use crate::Fail;

#[expect(
    clippy::cast_precision_loss,
    reason = "diagnostic estimates may round above the bounded budgets; comparisons use usize"
)]
pub(crate) fn exceeded(code: &str, limit: usize, observed: usize) -> Fail {
    let diagnostic = ve_graph::Diagnostic::error(code, "geometry or storage budget exceeded")
        .with_limit(limit as f64, observed as f64);
    let diagnostic = match suggestion(code, limit, observed) {
        Some(text) => diagnostic.suggest(text),
        None => diagnostic,
    };
    Box::new(diagnostic)
}

/// Triangle accounting over the parts of a result: what is stored (a shared mesh once, however many placements use it, so
/// memory and work follow it) and what would be drawn at full detail (every placement counted).
#[derive(Default)]
pub(crate) struct Triangles {
    stored: usize,
    drawn: usize,
    /// The output that draws the most, and how much, for the heavy-scene warning.
    heaviest: Option<(String, usize)>,
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
            self.stored = self.stored.saturating_add(own);
            drawn_here = drawn_here
                .saturating_add(own.saturating_mul(part.transforms.as_ref().map_or(1, Vec::len)));
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
            return Err(exceeded(
                "budget.triangles",
                quality.triangles(),
                self.stored,
            ));
        }
        if self.drawn > quality.drawn_triangles() {
            return Err(exceeded(
                "budget.drawn_triangles",
                quality.drawn_triangles(),
                self.drawn,
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
