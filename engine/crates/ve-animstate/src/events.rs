//! Which events a clip's time passes in one step.

/// The indices into `times` of the events passed as a clip's time goes from `from` to `to` (seconds into a clip `length` seconds long), in the order they
/// are passed. Each event fires once: the interval is `(from, to]`. With `wrapped` the clip looped in between, so the interval is `(from, length)` and
/// then `[0, to]` (an event at the clip's start fires as it starts over). A `from` below zero means the clip has just started, so an event at 0 fires.
#[must_use]
pub fn crossed(times: &[f32], length: f32, from: f32, to: f32, wrapped: bool) -> Vec<usize> {
    if times.is_empty() || !(length.is_finite() && length > 0.0 && from.is_finite() && to.is_finite()) {
        return Vec::new();
    }
    let mut hits: Vec<(f32, usize)> = Vec::new();
    if wrapped {
        let late = times.iter().enumerate().filter(|(_, t)| **t > from && **t < length);
        let early = times.iter().enumerate().filter(|(_, t)| **t >= 0.0 && **t <= to);
        let mut first: Vec<(f32, usize)> = late.map(|(i, t)| (*t, i)).collect();
        let mut second: Vec<(f32, usize)> = early.map(|(i, t)| (*t, i)).collect();
        first.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        second.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        first.extend(second);
        return first.into_iter().map(|(_, i)| i).collect();
    }
    if to <= from {
        return Vec::new();
    }
    hits.extend(times.iter().enumerate().filter(|(_, t)| **t > from && **t <= to).map(|(i, t)| (*t, i)));
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, i)| i).collect()
}
