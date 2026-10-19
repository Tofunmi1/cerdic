//! A generic character maker: a skeleton tree of tapered bones (plus a few blobs) becomes one smooth skinned body with a rig
//! and walk, run, idle, jump and fall clips, written as ordinary graph text so a person edits it like any other model.
//!
//! The same description makes an anime girl, a dragon or a snake. Nothing in the generator knows which: bones carry a
//! role (leg, arm, spine, tail, wing...) that picks their motion, and templates fill the tree from a few sliders.
mod body;
mod rig;
mod templates;

use serde::{Deserialize, Serialize};

pub use templates::{Plan, Sliders, template};

/// What a bone does when the creature moves. The role picks its clips; it has no effect on the shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Does not move on its own.
    #[default]
    Free,
    /// Sways side to side.
    Spine,
    /// Bobs a little.
    Neck,
    /// Bobs a little.
    Head,
    /// Swings back and forth; lower segments bend.
    Leg,
    /// Swings opposite to the leg on its side; lower segments bend.
    Arm,
    /// Undulates, each segment a little behind the one before.
    Tail,
    /// Flaps up and down.
    Wing,
}

/// One tapered bone: the skeleton joint, the body's tube around it, and a rig bone.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bone {
    /// Lowercase letters, digits and underscores.
    pub name: String,
    /// The bone it hangs from, or none for the root.
    #[serde(default)]
    pub parent: Option<String>,
    /// Where the bone starts and ends in metres; the creature stands on y = 0 and faces +z.
    pub head: [f64; 3],
    /// See `head`.
    pub tail: [f64; 3],
    /// Extra points the tube passes through between head and tail.
    #[serde(default)]
    pub via: Vec<[f64; 3]>,
    /// Tube radius at the head and at the tail, in metres.
    pub radius: [f64; 2],
    /// How it moves.
    #[serde(default)]
    pub role: Role,
    /// Defined on the +x side and mirrored to -x as `name_l` and `name_r`.
    #[serde(default)]
    pub pair: bool,
    /// Where in the stride it is, as a fraction of a cycle; the right side adds one half.
    #[serde(default)]
    pub phase: f64,
    /// How smoothly it blends into what it joins, in metres; 0 picks a size from its radius.
    #[serde(default)]
    pub blend: f64,
}

/// A rounded lump of body with no bone of its own (a head, a chest, a belly).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blob {
    /// Lowercase letters, digits and underscores.
    pub name: String,
    /// Centre in metres.
    pub center: [f64; 3],
    /// Radii along x, y and z.
    pub radii: [f64; 3],
    /// Defined on the +x side and mirrored.
    #[serde(default)]
    pub pair: bool,
    /// Blend size in metres; 0 picks one from the smallest radius.
    #[serde(default)]
    pub blend: f64,
}

/// A whole creature.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creature {
    /// Prefix of the generated node names; lowercase letters, digits and underscores.
    pub name: String,
    /// The skeleton.
    pub bones: Vec<Bone>,
    /// Extra body lumps.
    #[serde(default)]
    pub blobs: Vec<Blob>,
    /// Body colour, each 0 to 1.
    pub color: [f64; 3],
    /// Outline width in pixels; 0 draws none.
    #[serde(default = "default_outline")]
    pub outline: f64,
    /// Grid cells along the longest side of the body: more is finer and slower.
    #[serde(default = "default_resolution")]
    pub resolution: u32,
    /// Where the creature stands, in metres; every bone and the body move by this.
    #[serde(default)]
    pub at: [f64; 3],
}

fn default_outline() -> f64 {
    1.8
}

fn default_resolution() -> u32 {
    72
}

/// Why a creature cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatureError(pub String);

impl std::fmt::Display for CreatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CreatureError {}

fn refuse<T>(message: impl Into<String>) -> Result<T, CreatureError> {
    Err(CreatureError(message.into()))
}

/// Longest creature name, and longest bone or blob name. Node names are at most 32 characters, and the longest one the
/// generator writes is `<creature>_<bone>_l_p` (a left bone's parent link): 13 + 1 + 14 + 2 + 2 = 32.
const MAX_CREATURE_NAME: usize = 13;
const MAX_PART_NAME: usize = 14;

/// Lowercase letters, digits and underscores, starting with a letter, within `longest` characters. A name may not end
/// in an underscore and one character (`x_l`): the generator adds endings like that, and `x_l` would collide with the
/// left copy of `x`.
fn valid_name(name: &str, longest: usize) -> bool {
    let bytes = name.as_bytes();
    !name.is_empty()
        && name.len() <= longest
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !(bytes.len() >= 2 && bytes[bytes.len() - 2] == b'_')
}

impl Creature {
    /// Checks names, references, numbers and the mirrored side of every part.
    ///
    /// # Errors
    /// Names the first thing that is wrong and how to fix it.
    pub fn validate(&self) -> Result<(), CreatureError> {
        if !valid_name(&self.name, MAX_CREATURE_NAME) {
            return refuse(format!(
                "creature name must be 1 to {MAX_CREATURE_NAME} lowercase letters, digits or underscores, starting with a letter and not ending in an underscore and one character"
            ));
        }
        // The skin follows at most `rig::MAX_BONES` rig bones, and a paired bone is a left and a right one.
        let rig_bones: usize = self.bones.iter().map(|b| if b.pair { 2 } else { 1 }).sum();
        if rig_bones == 0 || rig_bones > crate::rig::MAX_BONES {
            return refuse(format!(
                "a creature has 1 to {} rig bones and this one has {rig_bones} (a paired bone counts twice): remove bones or make some unpaired",
                crate::rig::MAX_BONES
            ));
        }
        if !(16..=160).contains(&self.resolution) {
            return refuse("resolution must be between 16 and 160 cells");
        }
        if self.at.iter().any(|v| !v.is_finite() || v.abs() > 1000.0) {
            return refuse("`at` must be a position within 1000 m");
        }
        if !self.color.iter().all(|c| (0.0..=1.0).contains(c))
            || !(0.0..=16.0).contains(&self.outline)
        {
            return refuse("colour channels are 0 to 1 and the outline is 0 to 16 pixels");
        }
        let mut seen = self.check_bones()?;
        self.check_tree()?;
        self.check_blobs(&mut seen)
    }

    /// Each bone's name, points, radii and side; returns the names in use.
    fn check_bones(&self) -> Result<std::collections::BTreeSet<&str>, CreatureError> {
        let mut seen = std::collections::BTreeSet::new();
        for bone in &self.bones {
            if !valid_name(&bone.name, MAX_PART_NAME) || !seen.insert(bone.name.as_str()) {
                return refuse(format!(
                    "bone name `{}` is repeated or not valid: use 1 to {MAX_PART_NAME} lowercase letters, digits or underscores, starting with a letter and not ending in an underscore and one character",
                    bone.name
                ));
            }
            let points = std::iter::once(&bone.head)
                .chain(&bone.via)
                .chain(std::iter::once(&bone.tail));
            if points
                .clone()
                .any(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 100.0))
            {
                return refuse(format!(
                    "bone `{}` has a point that is not a finite number within 100 m",
                    bone.name
                ));
            }
            if bone.pair && points.clone().any(|p| p[0] < 0.0) {
                return refuse(format!(
                    "paired bone `{}` is defined on the +x side: its x values must not be negative",
                    bone.name
                ));
            }
            if bone
                .head
                .iter()
                .zip(&bone.tail)
                .all(|(a, b)| (a - b).abs() < 1e-9)
                && bone.via.is_empty()
            {
                return refuse(format!(
                    "bone `{}` has its head and tail at the same point",
                    bone.name
                ));
            }
            if !bone.radius.iter().all(|r| *r > 0.0 && *r <= 50.0) {
                return refuse(format!("bone `{}` needs positive radii", bone.name));
            }
            if !bone.phase.is_finite() || !bone.blend.is_finite() || bone.blend < 0.0 {
                return refuse(format!("bone `{}` has a bad phase or blend", bone.name));
            }
        }
        Ok(seen)
    }

    /// One root, parents that exist, listed first, on the right side, and a chain the engine can follow.
    fn check_tree(&self) -> Result<(), CreatureError> {
        let roots = self.bones.iter().filter(|b| b.parent.is_none()).count();
        if roots != 1 {
            return refuse(format!(
                "a creature has exactly one root bone (no parent), found {roots}"
            ));
        }
        for bone in &self.bones {
            if let Some(parent) = &bone.parent {
                let Some(found) = self.bones.iter().find(|b| &b.name == parent) else {
                    return refuse(format!(
                        "bone `{}` names a parent `{parent}` that does not exist",
                        bone.name
                    ));
                };
                if found.pair && !bone.pair {
                    return refuse(format!(
                        "bone `{}` is not paired but hangs from the paired bone `{parent}`",
                        bone.name
                    ));
                }
            }
        }
        // The engine lets a bone follow a chain of at most `MAX_DEPTH` parents.
        for index in 0..self.bones.len() {
            let mut depth = 0;
            let mut current = &self.bones[index];
            while let Some(parent) = current
                .parent
                .as_ref()
                .and_then(|p| self.bones.iter().find(|b| &b.name == p))
            {
                depth += 1;
                current = parent;
                if depth > crate::hierarchy::MAX_DEPTH {
                    return refuse(format!(
                        "bone `{}` is more than {} bones below the root: shorten the chain or hang part of it from a nearer bone",
                        self.bones[index].name,
                        crate::hierarchy::MAX_DEPTH
                    ));
                }
            }
        }
        // Parents must come before children so the rig is written in order, and there must be no cycle.
        for (index, bone) in self.bones.iter().enumerate() {
            if let Some(parent) = &bone.parent
                && !self.bones[..index].iter().any(|b| &b.name == parent)
            {
                return refuse(format!(
                    "bone `{}` is listed before its parent `{parent}`",
                    bone.name
                ));
            }
        }
        Ok(())
    }

    /// Each blob's name, centre and radii.
    fn check_blobs<'a>(
        &'a self,
        seen: &mut std::collections::BTreeSet<&'a str>,
    ) -> Result<(), CreatureError> {
        for blob in &self.blobs {
            if !valid_name(&blob.name, MAX_PART_NAME) || !seen.insert(blob.name.as_str()) {
                return refuse(format!(
                    "blob name `{}` repeats a bone or blob or is not valid: use 1 to {MAX_PART_NAME} lowercase letters, digits or underscores, starting with a letter and not ending in an underscore and one character",
                    blob.name
                ));
            }
            if !blob.radii.iter().all(|r| *r > 0.0 && *r <= 50.0)
                || blob.center.iter().any(|v| !v.is_finite())
            {
                return refuse(format!(
                    "blob `{}` needs a finite centre and positive radii",
                    blob.name
                ));
            }
            if blob.pair && blob.center[0] < 0.0 {
                return refuse(format!(
                    "paired blob `{}` is defined on the +x side",
                    blob.name
                ));
            }
        }
        Ok(())
    }

    /// Writes the creature as graph text: the blended body surface, the skeleton, the skin and the clips.
    ///
    /// # Errors
    /// The creature does not validate.
    pub fn to_text(&self) -> Result<String, CreatureError> {
        self.validate()?;
        let mut text = String::new();
        let solid = body::write(self, &mut text);
        rig::write(self, &solid, &mut text);
        Ok(text)
    }
}

/// What a tool call carries: a plan with sliders, or a whole hand-written creature.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    plan: Option<Plan>,
    #[serde(default)]
    sliders: Sliders,
    creature: Option<Creature>,
    /// Replaces the creature's name, so several can live in one document.
    name: Option<String>,
    /// Where it stands, in metres.
    at: Option<[f64; 3]>,
}

/// Makes the graph text for a request written as JSON: either `{"plan": "biped", "sliders": {...}}` or
/// `{"creature": {...}}`, with an optional `"name"`.
///
/// # Errors
/// The JSON is wrong, or the creature does not validate; the message says what to change.
pub fn make_from_json(json: &str) -> Result<String, CreatureError> {
    let request: Request = serde_json::from_str(json)
        .map_err(|e| CreatureError(format!("not a valid request: {e}")))?;
    let mut creature = match (request.plan, request.creature) {
        (Some(plan), None) => template(plan, &request.sliders)?,
        (None, Some(creature)) => creature,
        _ => {
            return refuse(
                "give either `plan` (with optional `sliders`) or a whole `creature`, not both and not neither",
            );
        }
    };
    if let Some(name) = request.name {
        creature.name = name;
    }
    if let Some(at) = request.at {
        creature.at = at;
    }
    creature.to_text()
}

/// A number written for the text language: four decimals, no trailing zeros.
pub(crate) fn num(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

pub(crate) fn vec3(value: [f64; 3]) -> String {
    format!("[{}, {}, {}]", num(value[0]), num(value[1]), num(value[2]))
}
