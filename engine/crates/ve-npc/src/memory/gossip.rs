use super::{Embedder, MemoryDraft, MemoryError, MemoryId, MemoryStore, Source};

impl MemoryStore {
    /// Shares a copy with provenance and decayed importance.
    ///
    /// # Errors
    /// Returns [`MemoryError::UnknownMemory`] when `id` is absent, or
    /// [`MemoryError::TooManyHops`] when the new provenance exceeds `max_hops`.
    pub fn share(
        &self,
        id: MemoryId,
        to: &mut MemoryStore,
        from_name: &str,
        now: f64,
        embedder: &dyn Embedder,
        max_hops: u8,
    ) -> Result<MemoryId, MemoryError> {
        let m = self.get(id).ok_or(MemoryError::UnknownMemory(id))?;
        let hops = match &m.source {
            Source::Told { hops, .. } => hops.saturating_add(1),
            _ => 1,
        };
        if hops > max_hops {
            return Err(MemoryError::TooManyHops {
                hops,
                max: max_hops,
            });
        }
        let mut d = MemoryDraft::new(m.kind.clone(), m.text.clone(), m.importance * 0.8);
        d.place.clone_from(&m.place);
        d.participants.clone_from(&m.participants);
        d.tags.clone_from(&m.tags);
        d.source = Source::Told {
            from: from_name.to_owned(),
            hops,
        };
        Ok(to.add(d, now, embedder))
    }
}
