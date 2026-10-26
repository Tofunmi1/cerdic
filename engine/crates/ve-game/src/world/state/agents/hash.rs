//! Canonical agent state bytes.
use super::{AgentState, Mode};

impl AgentState {
    pub(in crate::world::state) fn append_hash(&self, bytes: &mut Vec<u8>) {
        bytes.push(match self.mode {
            Mode::Patrol => 0,
            Mode::Chase => 1,
            Mode::Stunned => 2,
        });
        for value in [self.stun, self.unseen, self.planned] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in [self.patrol, self.cursor, self.path.len()] {
            bytes.extend_from_slice(&u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
        }
        bytes.push(u8::from(self.caught));
        bytes.push(u8::from(self.goal.is_some()));
        for p in self.goal.iter().chain(&self.path) {
            for value in p {
                bytes.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }
}
