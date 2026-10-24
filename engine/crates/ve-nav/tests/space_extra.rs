//! Additional correctness tests for the extensible navigation core.
#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "small deterministic test graphs"
)]
#![expect(
    clippy::many_single_char_names,
    reason = "compact graph and Dijkstra notation in test helpers"
)]
use ve_nav::{
    AreaId, CostFilter, Edge, EdgeKind, NavConfig, NavMesh, NavSpace, NodeId, SearchLimit,
    find_node_path, find_point_path,
};

struct Graph {
    p: Vec<[f32; 3]>,
    a: Vec<AreaId>,
    e: Vec<Vec<Edge>>,
}
impl NavSpace for Graph {
    fn node_count(&self) -> usize {
        self.p.len()
    }
    fn position(&self, n: NodeId) -> [f32; 3] {
        self.p[n as usize]
    }
    fn area(&self, n: NodeId) -> AreaId {
        self.a[n as usize]
    }
    fn neighbours(&self, n: NodeId, out: &mut Vec<Edge>) {
        out.extend(self.e[n as usize].iter().copied());
    }
    fn locate(&self, p: [f32; 3]) -> Option<NodeId> {
        self.p
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| dist(**a, p).total_cmp(&dist(**b, p)))
            .map(|(i, _)| i as NodeId)
    }
}
fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let x = a[0] - b[0];
    let y = a[1] - b[1];
    let z = a[2] - b[2];
    (x * x + y * y + z * z).sqrt()
}
fn dijkstra(g: &Graph, s: usize, t: usize, f: &CostFilter) -> f32 {
    let mut d = vec![f32::INFINITY; g.p.len()];
    let mut done = vec![false; g.p.len()];
    d[s] = 0.0;
    for _ in 0..g.p.len() {
        let u = (0..g.p.len())
            .filter(|&i| !done[i])
            .min_by(|&a, &b| d[a].total_cmp(&d[b]));
        let Some(u) = u else { break };
        if u == t {
            break;
        }
        done[u] = true;
        for e in &g.e[u] {
            if let Some(c) = f.edge_cost(g.a[e.to as usize], e) {
                d[e.to as usize] = d[e.to as usize].min(d[u] + c);
            }
        }
    }
    d[t]
}
fn graph() -> Graph {
    let n = 40;
    let p = (0..n)
        .map(|i| [i as f32, (i % 7) as f32, 0.0])
        .collect::<Vec<_>>();
    let a = vec![AreaId::GROUND; n];
    let mut e = vec![Vec::new(); n];
    let mut seed = 0x1234_5678_u32;
    for i in 0..n {
        for j in i + 1..n {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if seed.is_multiple_of(9) || j == i + 1 {
                let l = dist(p[i], p[j]);
                e[i].push(Edge {
                    to: j as u32,
                    length: l,
                    kind: EdgeKind::WALK,
                });
                e[j].push(Edge {
                    to: i as u32,
                    length: l,
                    kind: EdgeKind::WALK,
                });
            }
        }
    }
    Graph { p, a, e }
}
#[test]
fn seeded_graph_paths_match_dijkstra() {
    let g = graph();
    let f = CostFilter::new();
    for k in 0..20 {
        let s = k * 7 % 40;
        let t = (k * 13 + 3) % 40;
        let p = find_node_path(&g, &f, s as u32, t as u32, SearchLimit::default()).expect("route");
        assert!((p.cost - dijkstra(&g, s, t, &f)).abs() < 1e-4);
    }
}
#[test]
fn forbidden_start_area_can_be_left() {
    let mut g = graph();
    g.a[0] = AreaId(1);
    let mut f = CostFilter::new();
    f.only_areas(&[AreaId::GROUND]);
    assert!(find_node_path(&g, &f, 0, 3, SearchLimit::default()).is_ok());
}
#[test]
fn neighbours_append_to_output() {
    let g = graph();
    let mut out = vec![Edge {
        to: 39,
        length: 0.0,
        kind: EdgeKind::WALK,
    }];
    g.neighbours(0, &mut out);
    assert_eq!(out[0].to, 39);
    assert!(out.len() > 1);
}
#[test]
fn painted_mesh_area_survives_point_search() {
    let v = [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 0.0, 10.0],
        [0.0, 0.0, 10.0],
    ];
    let c = NavConfig::new(
        0.5,
        0.0,
        1.8,
        0.4,
        45.0,
        [0.0, -1.0, 0.0],
        [10.0, 3.0, 10.0],
    )
    .expect("config");
    let mut m = NavMesh::build(&v, &[0, 1, 2, 0, 2, 3], &c).expect("mesh");
    m.paint_area([0.0, -1.0, 0.0], [10.0, 3.0, 10.0], AreaId(7));
    let mut f = CostFilter::new();
    f.only_areas(&[AreaId(7)]);
    assert!(
        find_point_path(
            &m,
            &f,
            [1.0, 0.0, 1.0],
            [9.0, 0.0, 9.0],
            SearchLimit::default()
        )
        .is_ok()
    );
}
#[test]
fn subunit_area_multiplier_keeps_route_optimal() {
    let g = Graph {
        p: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
        ],
        a: vec![AreaId::GROUND, AreaId(1), AreaId::GROUND, AreaId::GROUND],
        e: vec![
            vec![
                Edge {
                    to: 1,
                    length: 1.414,
                    kind: EdgeKind::WALK,
                },
                Edge {
                    to: 2,
                    length: 1.5,
                    kind: EdgeKind::WALK,
                },
            ],
            vec![Edge {
                to: 3,
                length: 1.414,
                kind: EdgeKind::WALK,
            }],
            vec![Edge {
                to: 3,
                length: 1.5,
                kind: EdgeKind::WALK,
            }],
            vec![],
        ],
    };
    let mut f = CostFilter::new();
    f.set_area_cost(AreaId(1), 0.5);
    let p = find_node_path(&g, &f, 0, 3, SearchLimit::default()).expect("path");
    assert!((p.cost - dijkstra(&g, 0, 3, &f)).abs() < 1e-5);
    assert_eq!(p.nodes, vec![0, 1, 3]);
}
