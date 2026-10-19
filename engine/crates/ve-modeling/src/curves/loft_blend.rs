//! Smooth cubic blending between the sections of a loft.
use super::super::cache::Value;
use crate::Fail;
use std::rc::Rc;
use ve_kernel::ExecutionContext;

/// The rows a loft is built from. With one step per span they are the sections themselves; with more,
/// each matching vertex follows a cubic Hermite curve through its section positions (tangents from the
/// neighbouring sections, one-sided at the ends), so the surface is smooth through every section.
pub(super) fn rows(
    sections: &[&Rc<Value>],
    steps: usize,
    context: &ExecutionContext,
) -> Result<Vec<Vec<[f64; 2]>>, Fail> {
    let loops = sections
        .iter()
        .map(|section| section.loops[0].as_slice())
        .collect::<Vec<_>>();
    if steps <= 1 {
        return Ok(loops.iter().map(|ring| ring.to_vec()).collect());
    }
    let last = loops.len() - 1;
    let mut rows = Vec::with_capacity(last * steps + 1);
    for span in 0..last {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "loft cancelled while blending sections",
            ));
        }
        for step in 0..steps {
            let u = f64::from(u32::try_from(step).unwrap_or(0))
                / f64::from(u32::try_from(steps).unwrap_or(1));
            rows.push(blend_row(&loops, span, u));
        }
    }
    rows.push(loops[last].to_vec());
    Ok(rows)
}

fn blend_row(loops: &[&[[f64; 2]]], span: usize, u: f64) -> Vec<[f64; 2]> {
    let (u2, u3) = (u * u, u * u * u);
    let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
    let h10 = u3 - 2.0 * u2 + u;
    let h01 = -2.0 * u3 + 3.0 * u2;
    let h11 = u3 - u2;
    (0..loops[span].len())
        .map(|vertex| {
            let at = |section: usize| loops[section][vertex];
            let tangent = |section: usize| {
                let (before, after) = (
                    section.saturating_sub(1),
                    (section + 1).min(loops.len() - 1),
                );
                let width = f64::from(u32::try_from(after - before).unwrap_or(1));
                [0, 1].map(|axis| (at(after)[axis] - at(before)[axis]) / width)
            };
            let (start, end) = (at(span), at(span + 1));
            let (m0, m1) = (tangent(span), tangent(span + 1));
            [0, 1].map(|axis| h00 * start[axis] + h10 * m0[axis] + h01 * end[axis] + h11 * m1[axis])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::blend_row;

    #[test]
    fn blending_passes_through_the_sections_and_stays_between_equal_neighbours() {
        let ring = |radius: f64| vec![[radius, 0.0], [0.0, radius], [-radius, 0.0]];
        let (a, b, c) = (ring(1.0), ring(2.0), ring(1.0));
        let loops = [a.as_slice(), b.as_slice(), c.as_slice()];
        assert_eq!(blend_row(&loops, 0, 0.0), a);
        assert_eq!(blend_row(&loops, 1, 0.0), b);
        let middle = blend_row(&loops, 0, 0.5);
        assert!(middle[0][0] > 1.0 && middle[0][0] < 2.0);
        // Symmetric sections have a flat tangent at the belly, so the blend peaks at the section.
        assert!(blend_row(&loops, 0, 0.9)[0][0] < 2.0 + 1.0e-9);
    }
}
