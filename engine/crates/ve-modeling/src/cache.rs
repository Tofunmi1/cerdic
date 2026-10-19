//! LRU of exported immutable meshes. No native kernel objects survive in the cache.
use crate::Material;
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_kernel::Mesh;

#[derive(Clone)]
pub(crate) struct Curve3 {
    pub(crate) points: Vec<[f64; 3]>,
    pub(crate) closed: bool,
}

/// What one node produced: a solid's mesh, a profile's canonical f64 loops, or neither (a check).
#[derive(Clone)]
pub(crate) struct Value {
    pub(crate) mesh: Mesh,
    /// UV coordinates in triangle-corner order; empty when the geometry has no authored UVs.
    pub(crate) uvs: Vec<[f32; 2]>,
    pub(crate) volume: f64,
    pub(crate) loops: Vec<Vec<[f64; 2]>>,
    pub(crate) curve: Option<Curve3>,
    /// The surface a `solid.material` gave this solid; transforms and booleans keep their first
    /// input's, everything else starts without one.
    pub(crate) material: Option<Material>,
    /// Ordered runs of triangles with their own materials, when a boolean or move combined inputs
    /// of different materials; empty means the whole mesh uses `material`.
    pub(crate) regions: Vec<crate::regions::Region>,
    pub(crate) instances: Option<crate::instances::InstanceSet>,
    pub(crate) frames: crate::frames::Frames,
    /// The editable polygon mesh a `poly.*` node produced; `mesh` then holds its triangles.
    pub(crate) poly: Option<Rc<ve_geometry::PolyMesh>>,
    /// The elements a `poly.select` node chose, with the mesh they were chosen on.
    pub(crate) selection: Option<Rc<crate::poly_ops::Selection>>,
    /// Unmeshed spatial field value used by the `field.*` graph family.
    pub(crate) field: Option<Rc<ve_geometry::Field>>,
    /// Non-fatal, typed reports produced by geometry operators.
    pub(crate) warnings: Vec<crate::EvaluationWarning>,
    /// An authored light value. Lights share graph evaluation and cache keys with geometry.
    pub(crate) light: Option<crate::SceneLight>,
    /// An authored scene environment value from a `scene.environment` node.
    pub(crate) environment: Option<crate::SceneEnvironment>,
    /// Keyframed transform tracks from `anim.track` nodes, played together on the part.
    pub(crate) animation: Vec<crate::anim::Track>,
    /// The response from a `behavior.on` node, if the part has one.
    pub(crate) behavior: Option<crate::anim::Behavior>,
    /// The output name of the part this one is attached to (`anim.parent`); it moves with that part.
    pub(crate) parent: Option<String>,
    /// The rest segment of a bone part (`rig.bone`).
    pub(crate) bone: Option<crate::rig::Segment>,
    /// The bones that skin this shape (`rig.skin`); weights are added after evaluation.
    pub(crate) skin: Option<crate::rig::Skinning>,
    /// The chain of bones this part (the target) is reached by (`rig.ik`).
    pub(crate) ik: Option<crate::rig::IkChain>,
    /// The physics properties of this part (`phys.body`).
    pub(crate) body: Option<crate::physics::PhysBody>,
    /// The number a `value.*` node produced; it fills a number parameter of the nodes that link to it.
    pub(crate) number: Option<f64>,
}

impl Value {
    pub(crate) fn solid(mesh: Mesh, volume: f64) -> Self {
        Self {
            mesh,
            uvs: Vec::new(),
            volume,
            loops: Vec::new(),
            curve: None,
            material: None,
            regions: Vec::new(),
            instances: None,
            frames: BTreeMap::new(),
            poly: None,
            selection: None,
            field: None,
            warnings: Vec::new(),
            light: None,
            environment: None,
            animation: Vec::new(),
            behavior: None,
            parent: None,
            bone: None,
            skin: None,
            ik: None,
            body: None,
            number: None,
        }
    }

    pub(crate) fn profile(loops: Vec<Vec<[f64; 2]>>) -> Self {
        Self {
            mesh: Mesh::default(),
            uvs: Vec::new(),
            volume: 0.0,
            loops,
            curve: None,
            material: None,
            regions: Vec::new(),
            instances: None,
            frames: BTreeMap::new(),
            poly: None,
            selection: None,
            field: None,
            warnings: Vec::new(),
            light: None,
            environment: None,
            animation: Vec::new(),
            behavior: None,
            parent: None,
            bone: None,
            skin: None,
            ik: None,
            body: None,
            number: None,
        }
    }

    pub(crate) fn curve(curve: Curve3) -> Self {
        let mut value = Self::nothing();
        value.curve = Some(curve);
        value
    }

    pub(crate) fn light(light: crate::SceneLight) -> Self {
        let mut value = Self::nothing();
        value.light = Some(light);
        value
    }

    pub(crate) fn environment(environment: crate::SceneEnvironment) -> Self {
        let mut value = Self::nothing();
        value.environment = Some(environment);
        value
    }

    pub(crate) fn nothing() -> Self {
        Self::solid(Mesh::default(), 0.0)
    }

    /// A single number from a `value.*` node.
    pub(crate) fn number(number: f64) -> Self {
        let mut value = Self::nothing();
        value.number = Some(number);
        value
    }

    pub(crate) fn bytes(&self) -> usize {
        self.mesh.positions.capacity() * size_of::<[f32; 3]>()
            + self.mesh.indices.capacity() * size_of::<u32>()
            + self.uvs.capacity() * size_of::<[f32; 2]>()
            + self
                .loops
                .iter()
                .map(|l| l.capacity() * size_of::<[f64; 2]>() + size_of::<Vec<[f64; 2]>>())
                .sum::<usize>()
            + self
                .curve
                .as_ref()
                .map_or(0, |curve| curve.points.capacity() * size_of::<[f64; 3]>())
            + size_of::<Self>()
            + self.light.as_ref().map_or(0, |light| {
                light.name.capacity() + size_of::<crate::SceneLight>()
            })
            + self
                .environment
                .as_ref()
                .map_or(0, |_| size_of::<crate::SceneEnvironment>())
            + self.regions.capacity() * size_of::<crate::regions::Region>()
            + self
                .poly
                .as_ref()
                .map_or(0, |poly| crate::poly_ops::bytes(poly))
            + self
                .selection
                .as_ref()
                .map_or(0, |selection| selection.bytes())
            + self
                .field
                .as_ref()
                .map_or(0, |field| field.estimated_bytes())
            + self
                .warnings
                .iter()
                .map(|warning| {
                    warning.code.capacity() + warning.node.capacity() + warning.message.capacity()
                })
                .sum::<usize>()
            + self
                .frames
                .keys()
                .map(|name| name.capacity() + size_of::<crate::Frame>() + 128)
                .sum::<usize>()
            + self.instances.as_ref().map_or(0, |set| {
                set.transforms.capacity() * size_of::<glam::DMat4>() + set.prototype.bytes()
            })
    }
}

struct Entry {
    value: Rc<Value>,
    tick: u64,
    bytes: usize,
}

pub(crate) struct Cache {
    entries: BTreeMap<String, Entry>,
    bytes: usize,
    cap: usize,
    tick: u64,
}

impl Cache {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            bytes: 0,
            cap,
            tick: 0,
        }
    }
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn get(&mut self, key: &str) -> Option<Rc<Value>> {
        self.tick += 1;
        let entry = self.entries.get_mut(key)?;
        entry.tick = self.tick;
        Some(Rc::clone(&entry.value))
    }
    pub(crate) fn insert(&mut self, key: String, value: Rc<Value>) {
        let bytes = key.capacity() + value.bytes() + size_of::<Entry>() + 128;
        if bytes > self.cap {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes -= old.bytes;
        }
        while self.bytes.saturating_add(bytes) > self.cap {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.tick)
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(old) = self.entries.remove(&oldest) {
                self.bytes -= old.bytes;
            }
        }
        self.tick += 1;
        self.bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                value,
                tick: self.tick,
                bytes,
            },
        );
    }
}
