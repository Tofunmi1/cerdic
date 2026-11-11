// Scene queries and ragdolls: ray casts and overlaps for the world API, and bone-part ragdolls that
// take over a character's pose. Included into `physics`, so it shares that module's imports (`World` here is `ve_physics::World`).

use ve_physics::ragdoll::{Bone, BoneJoint, RagdollDesc, bone_pose, rest_frame};

/// Marks the part that carries a `phys.ragdoll` definition.
#[derive(Component, Clone)]
pub(crate) struct RagdollRig(pub(crate) Arc<ve_modeling::ragdoll::RagdollSpec>);

/// A ragdoll that has gone limp: its bodies, the bone parts they pose, and each bone's authored rest frame.
struct ActiveRagdoll {
    bodies: Vec<BodyId>,
    bones: Vec<String>,
    rest: Vec<([f32; 3], [f32; 4])>,
}

/// The part something belongs to: its output name and authored node (a ragdoll bone has no node of its own).
pub(crate) struct Found {
    pub(crate) part: String,
    pub(crate) node: String,
}

const NOT_RUNNING: &str =
    "physics is not running: it runs in play mode (press play, or call viewer_play_mode) and only in scenes with phys.body parts";

impl Physics {
    /// The part a physics body belongs to.
    fn owner(&self, id: BodyId) -> Option<Found> {
        if let Some(live) = self.bodies.iter().find(|live| live.id == id) {
            return Some(Found {
                part: live.spec.name.clone(),
                node: live.node.clone(),
            });
        }
        self.ragdolls.values().find_map(|ragdoll| {
            ragdoll.bodies.iter().position(|body| *body == id).map(|index| Found {
                part: ragdoll.bones[index].clone(),
                node: String::new(),
            })
        })
    }

    /// The first body a ray meets, with the part it belongs to.
    pub(crate) fn ray(
        &mut self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        mask: u32,
    ) -> Result<Option<(Found, ve_physics::RayHit)>, String> {
        let hit = self
            .world
            .as_mut()
            .ok_or(NOT_RUNNING)?
            .raycast(origin, direction, max_distance, mask);
        Ok(hit.and_then(|hit| self.owner(hit.body).map(|found| (found, hit))))
    }

    /// The parts that have a body inside a sphere, nearest the id order.
    pub(crate) fn overlap(&mut self, centre: [f32; 3], radius: f32, mask: u32) -> Result<Vec<Found>, String> {
        let ids = self
            .world
            .as_mut()
            .ok_or(NOT_RUNNING)?
            .overlap_sphere(centre, radius, mask);
        Ok(ids.into_iter().filter_map(|id| self.owner(id)).collect())
    }

    /// The surface of any physics body a small sphere is touching (for particles that bounce off crates and walls); `None` when physics is not running.
    pub(crate) fn surface(&mut self, centre: [f32; 3], radius: f32) -> Option<ve_physics::SurfaceHit> {
        self.world.as_mut()?.nearest_surface(centre, radius, u32::MAX)
    }

    /// The pose of every bone of a limp ragdoll this frame, by the bone part's output name.
    pub(crate) fn ragdoll_poses(&self) -> &HashMap<String, Mat4> {
        &self.ragdoll_poses
    }

    /// Makes a ragdoll go limp from the pose its character is in now. `poses` are this frame's animated poses; `impulse` (zero for none) is a shove
    /// given to the root bone (a shot, an explosion). Returns how many bones it has.
    pub(crate) fn start_ragdoll(
        &mut self,
        spec: &ve_modeling::ragdoll::RagdollSpec,
        poses: &crate::animation::PartPoses,
        impulse: [f32; 3],
    ) -> Result<usize, String> {
        if self.ragdolls.contains_key(&spec.name) {
            return Err(format!("the ragdoll \"{}\" is already limp", spec.name));
        }
        let world = self.world.as_mut().ok_or(NOT_RUNNING)?;
        // Each bone where the animation has it right now.
        let moved: Vec<([f32; 3], [f32; 3])> = spec
            .bones
            .iter()
            .zip(&spec.segments)
            .map(|(name, segment)| {
                let pose = poses
                    .matrix_of(name)
                    .map_or(Mat4::IDENTITY, |matrix| Mat4::from_cols_array(&matrix));
                (
                    pose.transform_point3(Vec3::from_array(segment.head)).to_array(),
                    pose.transform_point3(Vec3::from_array(segment.tail)).to_array(),
                )
            })
            .collect();
        let bones: Vec<Bone> = moved
            .iter()
            .enumerate()
            .map(|(index, (head, tail))| {
                let direction = (Vec3::from_array(*tail) - Vec3::from_array(*head)).normalize_or_zero();
                let radius = if spec.radius > 0.0 {
                    spec.radius
                } else {
                    ((Vec3::from_array(*tail) - Vec3::from_array(*head)).length() * 0.2).clamp(0.025, 0.15)
                };
                let parent = spec.parents.get(index).copied().flatten();
                let joint = if spec.hinges.get(index).copied().unwrap_or(false) {
                    // A bent bone bends in its own plane; a straight one about the chosen axis.
                    let bend = parent.map_or(Vec3::ZERO, |p| {
                        let before = (Vec3::from_array(moved[p].1) - Vec3::from_array(moved[p].0)).normalize_or_zero();
                        before.cross(direction)
                    });
                    let axis = if bend.length() > 0.2 {
                        bend.normalize().to_array()
                    } else {
                        spec.hinge_axis
                    };
                    BoneJoint::Hinge {
                        axis,
                        limits: spec.hinge_limits,
                    }
                } else {
                    BoneJoint::Ball {
                        swing: spec.swing,
                        twist: [-spec.twist, spec.twist],
                    }
                };
                Bone {
                    parent,
                    head: *head,
                    tail: *tail,
                    radius,
                    joint,
                }
            })
            .collect();
        let built = ve_physics::ragdoll::build(
            world,
            &RagdollDesc {
                bones,
                density: spec.density,
                friction: 0.8,
                self_collide: false,
                joint_damping: spec.damping,
            },
        )
        .map_err(|error| format!("the ragdoll \"{}\" could not be built: {error}", spec.name))?;
        if impulse.iter().any(|v| *v != 0.0)
            && let Some(root) = built.bodies.first()
            && let Ok(pose) = world.pose(*root)
        {
            // A failed shove (non-finite numbers) leaves the ragdoll to simply fall.
            let _ = world.apply_impulse_at(*root, impulse, pose.position);
        }
        // The bone parts are posed from the bodies relative to where each bone was authored.
        let rest = spec
            .segments
            .iter()
            .map(|segment| {
                rest_frame(&Bone {
                    parent: None,
                    head: segment.head,
                    tail: segment.tail,
                    radius: 0.05,
                    joint: BoneJoint::Fixed,
                })
            })
            .collect();
        let count = built.bodies.len();
        self.ragdolls.insert(
            spec.name.clone(),
            ActiveRagdoll {
                bodies: built.bodies,
                bones: spec.bones.clone(),
                rest,
            },
        );
        Ok(count)
    }
}

/// Poses every limp ragdoll's bone parts from its bodies.
fn update_ragdolls(world: &World, ragdolls: &HashMap<String, ActiveRagdoll>, poses: &mut HashMap<String, Mat4>) {
    poses.clear();
    for ragdoll in ragdolls.values() {
        for ((id, name), (rest_position, rest_rotation)) in ragdoll.bodies.iter().zip(&ragdoll.bones).zip(&ragdoll.rest) {
            if let Ok(pose) = world.pose(*id) {
                let matrix = bone_pose(*rest_position, *rest_rotation, pose.position, pose.rotation);
                poses.insert(name.clone(), Mat4::from_cols_array(&matrix));
            }
        }
    }
}
