//! Library programs written in SimL: the same text a model would write, shipped so cloth, hair and ropes are one `include` away.
//! They are plain source, not engine features; every parameter is an ordinary `param`.

/// A rope bridge: 41 planks in a chain between two pinned posts, each plank pulled to the spacing of its neighbours. `cut` is the number of
/// a link to snap (-1 keeps the bridge whole): set it at run time and the two halves swing down from their posts. The ends sit at `span`
/// apart, a little closer than the rope is long, so the bridge sags.
pub const BRIDGE: &str = "sim bridge
param gravity = vec3(0.0, -9.8, 0.0)
param damping = 0.992
param spacing = 0.55
param span = 19.6
param height = 5.0
param cut = -1.0
state pos: vec3[41]
state prev: vec3[41]
kernel init each i in pos {
  pos[i] = vec3((i / (n - 1.0) - 0.5) * span, height, 0.0)
  prev[i] = pos[i]
}
kernel predict each i in pos {
  let end = i == 0.0 || i == n - 1.0
  if !end {
    let v = (pos[i] - prev[i]) * damping
    prev[i] = pos[i]
    pos[i] += v + gravity * dt * dt
  }
}
kernel solve each i in pos {
  let end = i == 0.0 || i == n - 1.0
  let p = pos[i]
  if !end {
    let corr = vec3(0.0)
    let count = 0.0
    let left = max(i - 1.0, 0.0)
    let right = min(i + 1.0, n - 1.0)
    if abs(i - 1.0 - cut) > 0.5 {
      corr += pull(p, pos[left], spacing) * select(left == 0.0, 1.0, 0.5)
      count += 1.0
    }
    if abs(i - cut) > 0.5 {
      corr += pull(p, pos[right], spacing) * select(right == n - 1.0, 1.0, 0.5)
      count += 1.0
    }
    if count > 0.0 {
      p += corr * (1.4 / count)
    }
  }
  pos[i] = p
}
schedule predict; repeat 40 { solve }
output particles pos
";

/// Cloth: position-based dynamics with Jacobi relaxation over the mesh's edges (each point gathers the corrections of its own edges, so
/// every element writes only itself), points above `pin_height` riding the bone `anchor`, and a capsule collider along the bone `body`.
pub const CLOTH: &str = "sim cloth
param gravity = vec3(0.0, -9.8, 0.0)
param damping = 0.985
param relax = 1.3
param pin_height = 0.9
param collider_radius = 0.0
param collider_top = vec3(0.0, 0.3, 0.0)
param collider_bottom = vec3(0.0, -0.3, 0.0)
input mesh cloth
input bone anchor
input bone body
state pos: vec3[cloth]
state prev: vec3[cloth]
kernel init each i in pos {
  pos[i] = rest(cloth, i)
  prev[i] = rest(cloth, i)
}
kernel predict each i in pos {
  let pinned = rest(cloth, i).y > pin_height
  if pinned {
    let at = bone_point(anchor, rest(cloth, i))
    prev[i] = at
    pos[i] = at
  } else {
    let v = (pos[i] - prev[i]) * damping
    prev[i] = pos[i]
    pos[i] += v + gravity * dt * dt
  }
}
kernel solve each i in pos {
  let pinned = rest(cloth, i).y > pin_height
  let p = pos[i]
  if !pinned {
    let corr = vec3(0.0)
    let count = 0.0
    for j, len in adjacent(cloth, i) {
      let share = select(rest(cloth, j).y > pin_height, 1.0, 0.5)
      corr += pull(p, pos[j], len) * share
      count += 1.0
    }
    if count > 0.0 {
      p += corr * (relax / count)
    }
    let a = bone_point(body, collider_top)
    let b = bone_point(body, collider_bottom)
    let gap = sdf_capsule(p, a, b, collider_radius)
    if gap < 0.0 {
      p += normalize(p - closest_segment(p, a, b)) * (0.0 - gap)
    }
  }
  pos[i] = p
}
schedule predict; repeat 16 { solve }
output mesh pos
";
