
#[cfg(test)]
mod tests {
    use super::*;

    /// Unpainted weights.
    fn weigh_vertex(
        point: [f32; 3],
        segments: &[Segment],
        falloff: u32,
        influences: usize,
    ) -> ([u16; 4], [f32; 4]) {
        super::weigh_vertex(point, segments, falloff, influences, &[])
    }

    const LOWER: Segment = Segment {
        head: [0.0, -1.0, 0.0],
        tail: [0.0, 0.0, 0.0],
    };
    const UPPER: Segment = Segment {
        head: [0.0, 0.0, 0.0],
        tail: [0.0, 1.0, 0.0],
    };
    const IDENTITY: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    /// A quarter turn about +Z through the origin (the elbow), column-major.
    fn quarter_turn_z() -> [f32; 16] {
        [
            0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }

    #[test]
    fn weights_sum_to_one_follow_the_nearest_bone_and_blend_at_the_joint() {
        let segments = [LOWER, UPPER];
        let sum = |w: [f32; 4]| w.iter().sum::<f32>();
        for point in [
            [0.1, -0.9, 0.0],
            [0.0, 0.5, 0.2],
            [0.05, 0.0, 0.0],
            [3.0, 3.0, 3.0],
        ] {
            let (_, w) = weigh_vertex(point, &segments, 3, 4);
            assert!((sum(w) - 1.0).abs() < 1e-6, "{point:?}: {w:?}");
        }
        let (j, w) = weigh_vertex([0.1, -0.9, 0.0], &segments, 3, 4);
        assert_eq!(j[0], 0, "a vertex near the bottom follows the lower bone");
        assert!(w[0] > 0.95, "{w:?}");
        let (j, w) = weigh_vertex([0.0, 0.8, 0.0], &segments, 3, 4);
        assert_eq!(j[0], 1, "a vertex near the top follows the upper bone");
        assert!(w[0] > 0.95, "{w:?}");
        let (_, w) = weigh_vertex([0.1, 0.0, 0.0], &segments, 3, 4);
        assert!(
            (w[0] - 0.5).abs() < 1e-5 && (w[1] - 0.5).abs() < 1e-5,
            "equal distance blends evenly: {w:?}"
        );
        let (_, w) = weigh_vertex([0.1, 0.0, 0.0], &segments, 3, 1);
        assert!(
            (w[0] - 1.0).abs() < 1e-6 && w[1] == 0.0,
            "one influence: all weight on one bone"
        );
    }

    #[test]
    fn deformation_is_the_weighted_blend_of_the_bone_poses() {
        let poses = [IDENTITY, quarter_turn_z()];
        // Fully on the upper bone, a point one metre up turns to one metre along -X.
        let (p, n) = deform(
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1, 0, 0, 0],
            [1.0, 0.0, 0.0, 0.0],
            &poses,
        );
        assert!((p[0] + 1.0).abs() < 1e-6 && p[1].abs() < 1e-6, "{p:?}");
        assert!((n[0] + 1.0).abs() < 1e-6, "the normal turns with it: {n:?}");
        // Half and half: the midpoint of the two destinations, shorter than either (a bent joint).
        let (p, _) = deform(
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0, 1, 0, 0],
            [0.5, 0.5, 0.0, 0.0],
            &poses,
        );
        assert!(
            (p[0] + 0.5).abs() < 1e-6 && (p[1] - 0.5).abs() < 1e-6,
            "{p:?}"
        );
        // At rest nothing moves.
        let (p, n) = deform(
            [0.3, -0.2, 0.9],
            [0.0, 0.0, 1.0],
            [0, 1, 0, 0],
            [0.4, 0.6, 0.0, 0.0],
            &[IDENTITY, IDENTITY],
        );
        assert!(
            (p[0] - 0.3).abs() < 1e-6 && (p[2] - 0.9).abs() < 1e-6 && (n[2] - 1.0).abs() < 1e-6
        );
    }

    #[test]
    fn distance_to_a_segment_clamps_at_its_ends() {
        assert!(
            (distance([0.5, 0.5, 0.0], &UPPER) - 0.5).abs() < 1e-9,
            "beside the middle"
        );
        assert!(
            (distance([0.0, 3.0, 0.0], &UPPER) - 2.0).abs() < 1e-9,
            "past the tail"
        );
        assert!(
            (distance([0.0, -1.0, 0.0], &UPPER) - 1.0).abs() < 1e-9,
            "before the head"
        );
    }
}
