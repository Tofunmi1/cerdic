use super::{Human, place};
use std::mem::size_of;

/// Skeleton and surface landmarks in the evaluated body's coordinate spaces.
#[derive(Clone)]
pub(crate) struct Rig {
    /// Body turn about the vertical axis; gaze is authored in body space.
    pub(crate) turn: f32,
    /// World-space body placement.
    pub(crate) at: [f32; 3],
    /// Head vertices in grounded body space, used to fit hair.
    pub(crate) head_points: Vec<[f32; 3]>,
    /// Source point IDs that define the scalp.
    pub(crate) head_source_indices: Vec<usize>,
    /// Bone names, root first.
    pub(crate) bone_names: Vec<String>,
    /// Parent bone index, or -1 for the root.
    pub(crate) bone_parents: Vec<i32>,
    /// World-space bone heads.
    pub(crate) bone_heads: Vec<[f32; 3]>,
    /// World-space eye centres and radii.
    pub(crate) eyes: [([f32; 3], f32); 2],
    /// Source point IDs defining each eye, left then right.
    pub(crate) eye_source_indices: [Vec<usize>; 2],
}

impl Rig {
    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.head_points.capacity() * size_of::<[f32; 3]>()
            + self.head_source_indices.capacity() * size_of::<usize>()
            + self.bone_names.iter().map(String::capacity).sum::<usize>()
            + self.bone_parents.capacity() * size_of::<i32>()
            + self.bone_heads.capacity() * size_of::<[f32; 3]>()
            + self
                .eye_source_indices
                .iter()
                .map(|indices| indices.capacity() * size_of::<usize>())
                .sum::<usize>()
    }

    /// A grounded body point transformed into the body's world placement.
    pub(crate) fn world(&self, point: [f32; 3]) -> [f32; 3] {
        place(self.turn, self.at, point)
    }

    /// Refreshes landmarks from a deformed mesh mapped to human-pack source points.
    pub(crate) fn update_surface(&mut self, source_indices: &[u32], positions: &[[f32; 3]]) {
        let source_count = source_indices
            .iter()
            .copied()
            .max()
            .map_or(0, |source| source as usize + 1);
        let mut mapped = vec![None; source_count];
        for (source, position) in source_indices.iter().zip(positions) {
            if let Some(slot) = mapped.get_mut(*source as usize) {
                *slot = Some(*position);
            }
        }
        self.head_points = self
            .head_source_indices
            .iter()
            .filter_map(|source| mapped.get(*source).copied().flatten())
            .map(|point| unplace(self.turn, self.at, point))
            .collect();
        for (eye, sources) in self.eyes.iter_mut().zip(&self.eye_source_indices) {
            let points: Vec<_> = sources
                .iter()
                .filter_map(|source| mapped.get(*source).copied().flatten())
                .collect();
            if !points.is_empty() {
                *eye = ball(&points);
            }
        }
    }
}

pub(super) fn bone_source_indices(human: &Human, name: &str) -> Vec<usize> {
    let Some(index) = human.bone_names().iter().position(|bone| bone == name) else {
        return Vec::new();
    };
    human
        .skin()
        .iter()
        .enumerate()
        .filter(|(_, weights)| {
            weights
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .is_some_and(|&(bone, _)| usize::from(bone) == index)
        })
        .map(|(source, _)| source)
        .collect()
}

pub(super) fn ball_for_bone(human: &Human, positions: &[[f32; 3]], bone: &str) -> ([f32; 3], f32) {
    let points: Vec<_> = bone_source_indices(human, bone)
        .iter()
        .filter_map(|source| positions.get(*source).copied())
        .collect();
    if points.is_empty() {
        ([0.0; 3], 0.0)
    } else {
        ball(&points)
    }
}

fn ball(points: &[[f32; 3]]) -> ([f32; 3], f32) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the eye bone has a few hundred points"
    )]
    let count = points.len() as f32;
    let centre = [0, 1, 2].map(|axis| points.iter().map(|p| p[axis]).sum::<f32>() / count);
    let radius = points
        .iter()
        .map(|p| {
            (0..3)
                .map(|axis| (p[axis] - centre[axis]).powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .sum::<f32>()
        / count;
    (centre, radius)
}

fn unplace(turn: f32, at: [f32; 3], point: [f32; 3]) -> [f32; 3] {
    let (sin, cos) = turn.sin_cos();
    let (x, z) = (point[0] - at[0], point[2] - at[2]);
    [x * cos - z * sin, point[1] - at[1], x * sin + z * cos]
}

#[cfg(test)]
mod tests {
    use super::Rig;

    #[test]
    fn sculpt_updates_scalp_and_eye_landmarks_in_body_space() {
        let mut rig = Rig {
            turn: 0.0,
            at: [1.0, 2.0, 3.0],
            head_points: Vec::new(),
            head_source_indices: vec![0, 1],
            bone_names: Vec::new(),
            bone_parents: Vec::new(),
            bone_heads: Vec::new(),
            eyes: [([0.0; 3], 0.0); 2],
            eye_source_indices: [vec![2, 3], vec![4]],
        };
        rig.update_surface(
            &[0, 1, 2, 3, 4],
            &[
                [2.0, 3.0, 4.0],
                [4.0, 5.0, 6.0],
                [1.0, 2.0, 3.0],
                [3.0, 2.0, 3.0],
                [8.0, 9.0, 10.0],
            ],
        );
        assert_eq!(rig.head_points, vec![[1.0, 1.0, 1.0], [3.0, 3.0, 3.0]]);
        assert_eq!(rig.eyes[0], ([2.0, 2.0, 3.0], 1.0));
        assert_eq!(rig.eyes[1], ([8.0, 9.0, 10.0], 0.0));
    }
}
