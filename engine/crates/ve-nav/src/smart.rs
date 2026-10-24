//! Claimable, cooldown-aware slots for interactive world objects.
use thiserror::Error;

/// Smart object identifier, starting at one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u32);
/// Offset and facing for one interaction slot.
#[derive(Clone, Copy, Debug)]
pub struct SlotDef {
    /// Position offset.
    pub offset: [f32; 3],
    /// Facing in degrees.
    pub facing_deg: f32,
}
/// Smart object description.
#[derive(Clone, Debug)]
pub struct SmartObjectDef {
    /// Object category.
    pub kind: String,
    /// World position.
    pub position: [f32; 3],
    /// Available slots.
    pub slots: Vec<SlotDef>,
    /// Cooldown after a release.
    pub cooldown_secs: f64,
    /// Claim expiry duration.
    pub claim_ttl_secs: f64,
}
/// A held slot and its approach pose.
#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    /// Object id.
    pub object: ObjectId,
    /// Slot index.
    pub slot: usize,
    /// Holder name.
    pub who: String,
    /// Approach position.
    pub position: [f32; 3],
    /// Facing direction.
    pub facing_deg: f32,
}
/// Failure to find or change a claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ClaimError {
    /// Object id was absent.
    #[error("unknown object")]
    UnknownObject,
    /// Every slot is occupied.
    #[error("all slots are occupied")]
    Full,
    /// Available slots are cooling down.
    #[error("slots are cooling down")]
    Cooldown,
    /// Holder already owns a slot.
    #[error("holder already has a slot")]
    AlreadyClaimed,
    /// Claim does not identify a currently held slot.
    #[error("claim is not held")]
    NotClaimed,
}
#[derive(Clone)]
struct Held {
    claim: Claim,
    claimed_at: f64,
}
struct Object {
    def: SmartObjectDef,
    held: Vec<Option<Held>>,
    cooldown: Vec<f64>,
}
/// Deterministic store of smart objects and their claims.
#[derive(Default)]
pub struct SmartObjects {
    objects: Vec<Option<Object>>,
}
impl SmartObjects {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds an object and returns its identifier.
    pub fn add(&mut self, def: SmartObjectDef) -> ObjectId {
        let id = ObjectId(self.objects.len() as u32 + 1);
        let n = def.slots.len();
        self.objects.push(Some(Object {
            def,
            held: vec![None; n],
            cooldown: vec![0.0; n],
        }));
        id
    }
    /// Removes an object and all claims it owns.
    pub fn remove(&mut self, id: ObjectId) {
        if let Some(slot) =
            id.0.checked_sub(1)
                .and_then(|i| self.objects.get_mut(i as usize))
        {
            *slot = None;
        }
    }
    /// Finds available objects of a kind, nearest first with id tie-breaks.
    #[must_use]
    pub fn find(&self, kind: &str, near: [f32; 3], max_dist: f64, now: f64) -> Vec<ObjectId> {
        let mut found: Vec<(f64, ObjectId)> = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(i, o)| {
                let o = o.as_ref()?;
                if o.def.kind != kind || !available(o, now) {
                    return None;
                }
                let d = dist(o.def.position, near);
                (d <= max_dist).then_some((d, ObjectId(i as u32 + 1)))
            })
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        found.into_iter().map(|x| x.1).collect()
    }
    /// Claims the first free slot that is not cooling down.
    ///
    /// # Errors
    /// Returns `UnknownObject`, `AlreadyClaimed`, `Full` or `Cooldown` as applicable.
    pub fn claim(&mut self, id: ObjectId, who: &str, now: f64) -> Result<Claim, ClaimError> {
        let o = self.object_mut(id)?;
        if o.held.iter().flatten().any(|h| h.claim.who == who) {
            return Err(ClaimError::AlreadyClaimed);
        }
        let free: Vec<usize> = o
            .held
            .iter()
            .enumerate()
            .filter_map(|(i, h)| h.is_none().then_some(i))
            .collect();
        if free.is_empty() {
            return Err(ClaimError::Full);
        }
        let slot = free
            .into_iter()
            .find(|&i| o.cooldown[i] <= now)
            .ok_or(ClaimError::Cooldown)?;
        let s = o.def.slots[slot];
        let p = o.def.position;
        let claim = Claim {
            object: id,
            slot,
            who: who.to_owned(),
            position: [p[0] + s.offset[0], p[1] + s.offset[1], p[2] + s.offset[2]],
            facing_deg: s.facing_deg,
        };
        o.held[slot] = Some(Held {
            claim: claim.clone(),
            claimed_at: now,
        });
        Ok(claim)
    }
    /// Releases a held slot and starts its cooldown.
    ///
    /// # Errors
    /// Returns `UnknownObject` or `NotClaimed` when the claim is not current.
    pub fn release(&mut self, claim: &Claim, now: f64) -> Result<(), ClaimError> {
        let o = self.object_mut(claim.object)?;
        let held = o.held.get_mut(claim.slot).ok_or(ClaimError::NotClaimed)?;
        if held.as_ref().is_none_or(|h| h.claim.who != claim.who) {
            return Err(ClaimError::NotClaimed);
        }
        *held = None;
        o.cooldown[claim.slot] = now + o.def.cooldown_secs;
        Ok(())
    }
    /// Drops claims strictly older than their object's TTL.
    pub fn tick(&mut self, now: f64) -> Vec<Claim> {
        let mut expired = Vec::new();
        for o in self.objects.iter_mut().flatten() {
            for h in &mut o.held {
                if h.as_ref()
                    .is_some_and(|held| now - held.claimed_at > o.def.claim_ttl_secs)
                    && let Some(held) = h.take()
                {
                    expired.push(held.claim);
                }
            }
        }
        expired
    }
    /// Lists claims in object-id then slot order.
    #[must_use]
    pub fn claimed_by(&self, who: &str) -> Vec<Claim> {
        self.objects
            .iter()
            .flatten()
            .flat_map(|o| o.held.iter().flatten())
            .filter(|h| h.claim.who == who)
            .map(|h| h.claim.clone())
            .collect()
    }
    fn object_mut(&mut self, id: ObjectId) -> Result<&mut Object, ClaimError> {
        id.0.checked_sub(1)
            .and_then(|i| self.objects.get_mut(i as usize))
            .and_then(Option::as_mut)
            .ok_or(ClaimError::UnknownObject)
    }
}
fn available(o: &Object, now: f64) -> bool {
    o.held
        .iter()
        .enumerate()
        .any(|(i, h)| h.is_none() && o.cooldown[i] <= now)
}
fn dist(from: [f32; 3], to: [f32; 3]) -> f64 {
    let dx = f64::from(from[0] - to[0]);
    let dy = f64::from(from[1] - to[1]);
    let dz = f64::from(from[2] - to[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}
