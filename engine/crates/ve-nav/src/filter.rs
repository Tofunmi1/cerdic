//! Per-query area and traversal costs.

use crate::{AreaId, Edge, EdgeKind};

/// Which areas and edge kinds an agent may use, and their cost multipliers.
#[derive(Clone, Debug)]
pub struct CostFilter {
    areas: u64,
    kinds: u64,
    area_costs: [f32; 64],
    kind_costs: [f32; 64],
}

impl CostFilter {
    /// Allows every area and walking at unit cost.
    #[must_use]
    pub fn new() -> Self {
        let mut kinds = 0_u64;
        kinds |= 1_u64 << EdgeKind::WALK.0;
        Self {
            areas: u64::MAX,
            kinds,
            area_costs: [1.0; 64],
            kind_costs: [1.0; 64],
        }
    }
    /// Excludes an area.
    pub fn exclude_area(&mut self, area: AreaId) {
        if area.0 < 64 {
            self.areas &= !(1_u64 << area.0);
        }
    }
    /// Allows exactly the listed areas.
    pub fn only_areas(&mut self, areas: &[AreaId]) {
        self.areas = 0;
        for area in areas {
            if area.0 < 64 {
                self.areas |= 1_u64 << area.0;
            }
        }
    }
    /// Sets an area multiplier; invalid values are ignored.
    pub fn set_area_cost(&mut self, area: AreaId, cost: f32) {
        if area.0 < 64 && cost.is_finite() && cost >= 0.0 {
            self.area_costs[area.0 as usize] = cost;
        }
    }
    /// Allows an edge kind with a multiplier; invalid values are ignored.
    pub fn allow_kind(&mut self, kind: EdgeKind, cost: f32) {
        if kind.0 < 64 && cost.is_finite() && cost >= 0.0 {
            self.kinds |= 1_u64 << kind.0;
            self.kind_costs[kind.0 as usize] = cost;
        }
    }
    /// Whether this area may be entered.
    #[must_use]
    pub fn allows_area(&self, area: AreaId) -> bool {
        area.0 < 64 && self.areas & (1_u64 << area.0) != 0
    }
    /// Whether this edge kind may be traversed.
    #[must_use]
    pub fn allows_kind(&self, kind: EdgeKind) -> bool {
        kind.0 < 64 && self.kinds & (1_u64 << kind.0) != 0
    }
    /// Area cost multiplier (1.0 for out-of-range ids).
    #[must_use]
    pub fn area_cost(&self, area: AreaId) -> f32 {
        self.area_costs.get(area.0 as usize).copied().unwrap_or(1.0)
    }
    /// Edge-kind cost multiplier (1.0 for out-of-range ids).
    #[must_use]
    pub fn kind_cost(&self, kind: EdgeKind) -> f32 {
        self.kind_costs.get(kind.0 as usize).copied().unwrap_or(1.0)
    }
    /// Effective edge cost, or `None` when its destination or kind is forbidden.
    #[must_use]
    pub fn edge_cost(&self, to_area: AreaId, edge: &Edge) -> Option<f32> {
        (self.allows_area(to_area) && self.allows_kind(edge.kind))
            .then(|| edge.length * self.area_cost(to_area) * self.kind_cost(edge.kind))
    }
}

impl Default for CostFilter {
    fn default() -> Self {
        Self::new()
    }
}
