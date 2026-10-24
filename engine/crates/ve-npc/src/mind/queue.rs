use super::{Ask, Mind, Reason, Refusal, Request, RequestId};

#[derive(Clone)]
pub(super) struct Entry {
    pub(super) request: Request,
    pub(super) estimate: u32,
    pub(super) taken: bool,
    pub(super) admitted: bool,
}

impl Mind {
    /// Requests deliberation and admits or queues it within budgets.
    pub fn ask(&mut self, npc: &str, reason: Reason, prompt: &str, now: f64) -> Ask {
        let recent: Vec<&Entry> = self
            .entries
            .values()
            .filter(|entry| entry.request.issued_at >= now - 60.0)
            .collect();
        if recent
            .iter()
            .filter(|entry| entry.request.npc == npc)
            .count()
            >= self.budget.calls_per_minute as usize
        {
            return Ask::Refused(Refusal::RateLimited);
        }
        let estimate = u32::try_from(prompt.chars().count().div_ceil(4)).unwrap_or(u32::MAX);
        let used: u64 = recent.iter().map(|entry| u64::from(entry.estimate)).sum();
        if used + u64::from(estimate) > u64::from(self.budget.tokens_per_minute) {
            return Ask::Refused(Refusal::OverTokenBudget);
        }
        let id = RequestId(self.next);
        self.next = self.next.saturating_add(1);
        let request = Request {
            id,
            npc: npc.into(),
            prompt: prompt.into(),
            reason,
            priority: reason.priority(),
            issued_at: now,
        };
        let admitted = self.entries.values().filter(|entry| entry.admitted).count()
            < self.budget.max_concurrent;
        self.entries.insert(
            id,
            Entry {
                request,
                estimate,
                taken: false,
                admitted,
            },
        );
        if admitted {
            Ask::Sent(id)
        } else {
            Ask::Queued(id)
        }
    }

    pub(super) fn admit_waiting(&mut self) {
        let slots = self
            .budget
            .max_concurrent
            .saturating_sub(self.entries.values().filter(|entry| entry.admitted).count());
        let mut ids: Vec<_> = self
            .entries
            .values()
            .filter(|entry| !entry.admitted)
            .map(|entry| (std::cmp::Reverse(entry.request.priority), entry.request.id))
            .collect();
        ids.sort();
        for (_, id) in ids.into_iter().take(slots) {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.admitted = true;
            }
        }
    }

    /// Returns admitted requests once, by priority then request id.
    pub fn take_outbox(&mut self) -> Vec<Request> {
        self.take_outbox_limit(usize::MAX)
    }

    pub(crate) fn take_outbox_limit(&mut self, limit: usize) -> Vec<Request> {
        let mut ids: Vec<_> = self
            .entries
            .values()
            .filter(|entry| entry.admitted && !entry.taken)
            .map(|entry| entry.request.id)
            .collect();
        ids.sort_by_key(|id| {
            let entry = &self.entries[id];
            (std::cmp::Reverse(entry.request.priority), *id)
        });
        ids.into_iter()
            .take(limit)
            .filter_map(|id| {
                self.entries.get_mut(&id).map(|entry| {
                    entry.taken = true;
                    entry.request.clone()
                })
            })
            .collect()
    }
}
