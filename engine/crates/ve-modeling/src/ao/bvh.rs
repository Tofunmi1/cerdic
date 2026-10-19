//! Median-split triangle BVH and fixed-sequence cosine-hemisphere visibility sampling.
use crate::Part;

const LEAF_TRIANGLES: usize = 8;
const RAY_DISTANCE: f64 = 6.0;
const RAY_BIAS: f64 = 0.001;
const DIRECTIONS: [[f64; 3]; 32] = [
    [0.992_156_741_6, 0.0, 0.125],
    [-0.976_281_209_5, 0.0, 0.216_506_350_9],
    [0.0, 0.960_143_218_5, 0.279_508_497_2],
    [0.0, -0.943_729_304_4, 0.330_718_913_9],
    [0.655_505_530_1, 0.655_505_530_1, 0.375],
    [-0.643_476_883_8, -0.643_476_883_8, 0.414_578_098_8],
    [-0.631_219_058_6, 0.631_219_058_6, 0.450_693_909_4],
    [0.618_718_433_5, -0.618_718_433_5, 0.484_122_918_3],
    [0.791_724_870_9, 0.327_943_179_2, 0.515_388_203_2],
    [-0.774_696_539_1, -0.320_889_813_2, 0.544_862_367_9],
    [-0.313_677_885_3, 0.757_285_404_8, 0.572_821_961_9],
    [0.306_296_195_1, -0.739_464_428_4, 0.599_478_940_4],
    [0.298_732_158_6, 0.721_203_228_9, 0.625],
    [-0.290_971_555_4, -0.702_467_475_4, 0.649_519_052_8],
    [-0.683_218_128, 0.282_998_214_7, 0.673_145_600_9],
    [0.663_410_481_5, -0.274_793_618_9, 0.695_970_545_4],
    [0.682_597_666_5, 0.135_777_117_8, 0.718_070_330_8],
    [-0.660_211_296_9, -0.131_324_192, 0.739_509_972_9],
    [-0.126_714_881_2, 0.637_038_726_4, 0.760_345_316_3],
    [0.121_931_451_3, -0.612_990_800_3, 0.780_624_749_8],
    [0.333_052_654_6, 0.498_448_522_2, 0.800_390_529_7],
    [-0.318_242_830_8, -0.476_284_054_6, 0.819_679_815_5],
    [-0.453_036_501_8, 0.302_709_312_7, 0.838_525_491_6],
    [0.428_529_629_5, -0.286_334_344_1, 0.856_956_825_1],
    [0.402_533_495_2, 0.268_964_282_5, 0.875],
    [-0.374_738_290_1, -0.250_392_120_3, 0.892_678_553_6],
    [-0.230_327_251, 0.344_709_091_1, 0.910_013_736_2],
    [0.208_338_837_4, -0.311_801_104_6, 0.927_024_810_9],
    [0.064_520_059_4, 0.324_364_242_7, 0.943_729_304_4],
    [-0.054_529_402_7, -0.274_137_819_8, 0.960_143_218_5],
    [-0.212_346_242_1, 0.042_238_293_7, 0.976_281_209_5],
    [0.122_598_160_1, -0.024_386_290_3, 0.992_156_741_6],
];

#[derive(Clone, Copy)]
struct Triangle {
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
    min: [f64; 3],
    max: [f64; 3],
    center: [f64; 3],
}

#[derive(Clone, Copy, Default)]
struct Node {
    min: [f64; 3],
    max: [f64; 3],
    first: usize,
    count: usize,
    left: usize,
    right: usize,
}

pub(super) struct Bvh {
    triangles: Vec<Triangle>,
    order: Vec<usize>,
    nodes: Vec<Node>,
}

impl Bvh {
    pub(super) fn from_parts(parts: &[Part]) -> Self {
        let mut triangles = Vec::new();
        for part in parts {
            for face in part.indices.chunks_exact(3) {
                let [Some(a), Some(b), Some(c)] = [
                    part.positions.get(face[0] as usize),
                    part.positions.get(face[1] as usize),
                    part.positions.get(face[2] as usize),
                ] else {
                    continue;
                };
                let (a, b, c) = (to64(*a), to64(*b), to64(*c));
                let min = min3(min3(a, b), c);
                let max = max3(max3(a, b), c);
                triangles.push(Triangle {
                    a,
                    b,
                    c,
                    min,
                    max,
                    center: mul(add(add(a, b), c), 1.0 / 3.0),
                });
            }
        }
        let mut tree = Self {
            order: (0..triangles.len()).collect(),
            triangles,
            nodes: Vec::new(),
        };
        if !tree.order.is_empty() {
            tree.build(0, tree.order.len());
        }
        tree
    }

    fn build(&mut self, first: usize, count: usize) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node::default());
        let (min, max) = self.bounds(first, count);
        if count <= LEAF_TRIANGLES {
            self.nodes[index] = Node {
                min,
                max,
                first,
                count,
                ..Node::default()
            };
            return index;
        }
        let extent = sub(max, min);
        let axis = if extent[0] >= extent[1] && extent[0] >= extent[2] {
            0
        } else if extent[1] >= extent[2] {
            1
        } else {
            2
        };
        self.order[first..first + count].sort_by(|a, b| {
            self.triangles[*a].center[axis]
                .total_cmp(&self.triangles[*b].center[axis])
                .then_with(|| a.cmp(b))
        });
        let left_count = count / 2;
        let left = self.build(first, left_count);
        let right = self.build(first + left_count, count - left_count);
        self.nodes[index] = Node {
            min,
            max,
            first: 0,
            count: 0,
            left,
            right,
        };
        index
    }

    fn bounds(&self, first: usize, count: usize) -> ([f64; 3], [f64; 3]) {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for id in &self.order[first..first + count] {
            min = min3(min, self.triangles[*id].min);
            max = max3(max, self.triangles[*id].max);
        }
        (min, max)
    }

    pub(super) fn bake_vertices(
        &self,
        positions: &[[f32; 3]],
        indices: &[u32],
        rays: usize,
    ) -> Vec<f32> {
        let mut normals = vec![[0.0; 3]; positions.len()];
        for face in indices.chunks_exact(3) {
            let (Some(&a), Some(&b), Some(&c)) = (
                positions.get(face[0] as usize),
                positions.get(face[1] as usize),
                positions.get(face[2] as usize),
            ) else {
                continue;
            };
            let n = cross(sub(to64(b), to64(a)), sub(to64(c), to64(a)));
            for id in face {
                if let Some(normal) = normals.get_mut(*id as usize) {
                    *normal = add(*normal, n);
                }
            }
        }
        positions
            .iter()
            .zip(normals)
            .map(|(position, normal)| {
                let normal = normalize(normal);
                if dot(normal, normal) <= 1.0e-24 || self.nodes.is_empty() {
                    return 1.0;
                }
                let tangent = normalize(cross(
                    if normal[2].abs() < 0.9 {
                        [0.0, 0.0, 1.0]
                    } else {
                        [0.0, 1.0, 0.0]
                    },
                    normal,
                ));
                let bitangent = cross(normal, tangent);
                let origin = add(to64(*position), mul(normal, RAY_BIAS));
                let stride = DIRECTIONS.len() / rays;
                let hits = DIRECTIONS
                    .iter()
                    .step_by(stride)
                    .take(rays)
                    .filter(|d| {
                        let direction = normalize(add(
                            add(mul(tangent, d[0]), mul(bitangent, d[1])),
                            mul(normal, d[2]),
                        ));
                        self.hit(origin, direction)
                    })
                    .count();
                let hit_count = u8::try_from(hits).unwrap_or_default();
                let ray_count = u8::try_from(rays).unwrap_or(1);
                1.0 - 0.85 * f32::from(hit_count) / f32::from(ray_count)
            })
            .collect()
    }

    fn hit(&self, origin: [f64; 3], direction: [f64; 3]) -> bool {
        let mut stack = [0_usize; 64];
        let mut len = 1;
        while len > 0 {
            len -= 1;
            let node = self.nodes[stack[len]];
            if !box_hit(origin, direction, node.min, node.max) {
                continue;
            }
            if node.count > 0 {
                for id in &self.order[node.first..node.first + node.count] {
                    let tri = self.triangles[*id];
                    if triangle_hit(origin, direction, tri) {
                        return true;
                    }
                }
            } else if len + 2 <= stack.len() {
                stack[len] = node.left;
                stack[len + 1] = node.right;
                len += 2;
            }
        }
        false
    }
}

fn triangle_hit(origin: [f64; 3], direction: [f64; 3], triangle: Triangle) -> bool {
    let first_edge = sub(triangle.b, triangle.a);
    let second_edge = sub(triangle.c, triangle.a);
    let perpendicular = cross(direction, second_edge);
    let determinant = dot(first_edge, perpendicular);
    if determinant.abs() < 1.0e-12 {
        return false;
    }
    let inverse = 1.0 / determinant;
    let offset = sub(origin, triangle.a);
    let barycentric_u = dot(offset, perpendicular) * inverse;
    if !(0.0..=1.0).contains(&barycentric_u) {
        return false;
    }
    let crossed = cross(offset, first_edge);
    let barycentric_v = dot(direction, crossed) * inverse;
    let distance = dot(second_edge, crossed) * inverse;
    barycentric_v >= 0.0
        && barycentric_u + barycentric_v <= 1.0
        && distance > RAY_BIAS * 2.0
        && distance <= RAY_DISTANCE
}

fn box_hit(origin: [f64; 3], direction: [f64; 3], min: [f64; 3], max: [f64; 3]) -> bool {
    let (mut near, mut far) = (0.0_f64, RAY_DISTANCE);
    for axis in 0..3 {
        if direction[axis].abs() < 1.0e-12 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return false;
            }
        } else {
            let inverse = 1.0 / direction[axis];
            let near_axis = (min[axis] - origin[axis]) * inverse;
            let far_axis = (max[axis] - origin[axis]) * inverse;
            near = near.max(near_axis.min(far_axis));
            far = far.min(near_axis.max(far_axis));
            if near > far {
                return false;
            }
        }
    }
    true
}

fn to64(v: [f32; 3]) -> [f64; 3] {
    v.map(f64::from)
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    if length <= 1.0e-24 {
        [0.0; 3]
    } else {
        mul(v, 1.0 / length)
    }
}
fn min3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}
fn max3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}
