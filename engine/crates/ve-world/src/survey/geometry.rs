//! Triangle arithmetic and a median-split ray acceleration tree.
use super::Triangle;

pub(super) fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
pub(super) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(super) fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    a.map(|v| v * k)
}
pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub(super) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(super) fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
pub(super) fn unit(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / length(a).max(1e-15))
}
#[expect(
    clippy::cast_precision_loss,
    reason = "sample and camera counts fit in f64's integer precision"
)]
pub(super) fn number(n: usize) -> f64 {
    n as f64
}

pub(super) struct Face {
    pub triangle: Triangle,
    pub normal: [f64; 3],
    pub area: f64,
}
pub(super) struct Scene {
    pub faces: Vec<Face>,
    root: Option<Node>,
    pub min: [f64; 3],
    pub max: [f64; 3],
}
struct Node {
    min: [f64; 3],
    max: [f64; 3],
    kind: Branch,
}
enum Branch {
    Leaf(Vec<usize>),
    Split(Box<Node>, Box<Node>),
}

impl Scene {
    pub(super) fn new(triangles: &[Triangle]) -> Self {
        let faces: Vec<_> = triangles
            .iter()
            .filter_map(|t| {
                if !t.iter().flatten().all(|v| v.is_finite()) {
                    return None;
                }
                let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
                let twice_area = length(n);
                (twice_area > 1e-12 && twice_area.is_finite()).then(|| Face {
                    triangle: *t,
                    normal: scale(n, 1.0 / twice_area),
                    area: twice_area * 0.5,
                })
            })
            .collect();
        let root = (!faces.is_empty()).then(|| Node::build(&faces, (0..faces.len()).collect()));
        let min = root.as_ref().map_or([0.0; 3], |n| n.min);
        let max = root.as_ref().map_or([0.0; 3], |n| n.max);
        Self {
            faces,
            root,
            min,
            max,
        }
    }

    pub(super) fn ray(&self, origin: [f64; 3], direction: [f64; 3], limit: f64) -> Option<f64> {
        let mut nearest = limit;
        let hit = self
            .root
            .as_ref()?
            .hit(&self.faces, origin, direction, &mut nearest);
        hit.then_some(nearest)
    }
}

impl Node {
    fn build(faces: &[Face], mut ids: Vec<usize>) -> Self {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for &id in &ids {
            for p in faces[id].triangle {
                for i in 0..3 {
                    min[i] = min[i].min(p[i]);
                    max[i] = max[i].max(p[i]);
                }
            }
        }
        let kind = if ids.len() <= 8 {
            Branch::Leaf(ids)
        } else {
            let mut axis = 0;
            for i in 1..3 {
                if max[i] - min[i] > max[axis] - min[axis] {
                    axis = i;
                }
            }
            let centre = |id: usize| {
                faces[id]
                    .triangle
                    .iter()
                    .map(|p| p[axis] / 3.0)
                    .sum::<f64>()
            };
            let middle = ids.len() / 2;
            ids.select_nth_unstable_by(middle, |a, b| {
                centre(*a).total_cmp(&centre(*b)).then(a.cmp(b))
            });
            let right = ids.split_off(middle);
            Branch::Split(
                Box::new(Self::build(faces, ids)),
                Box::new(Self::build(faces, right)),
            )
        };
        Self { min, max, kind }
    }

    fn hit(
        &self,
        faces: &[Face],
        origin: [f64; 3],
        direction: [f64; 3],
        nearest: &mut f64,
    ) -> bool {
        let mut near: f64 = 0.0;
        let mut far = *nearest;
        for i in 0..3 {
            if direction[i].abs() < 1e-15 {
                if origin[i] < self.min[i] - 1e-9 || origin[i] > self.max[i] + 1e-9 {
                    return false;
                }
            } else {
                let a = (self.min[i] - origin[i]) / direction[i];
                let b = (self.max[i] - origin[i]) / direction[i];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far + 1e-9 {
                    return false;
                }
            }
        }
        match &self.kind {
            Branch::Split(left, right) => {
                let a = left.hit(faces, origin, direction, nearest);
                right.hit(faces, origin, direction, nearest) || a
            }
            Branch::Leaf(ids) => {
                let mut hit = false;
                for &id in ids {
                    if let Some(t) = intersect(faces[id].triangle, origin, direction)
                        && t < *nearest
                    {
                        *nearest = t;
                        hit = true;
                    }
                }
                hit
            }
        }
    }
}

fn intersect(t: Triangle, origin: [f64; 3], direction: [f64; 3]) -> Option<f64> {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let h = cross(direction, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = sub(origin, t[0]);
    let u = dot(s, h) / det;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(direction, q) / det;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let distance = dot(e2, q) / det;
    (distance > 1e-7).then_some(distance)
}
