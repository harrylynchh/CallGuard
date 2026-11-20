//! All active and recent calls, in memory only.

use std::sync::Arc;

use dashmap::DashMap;

use super::record::{CallRecord, CallSource, CallSummary};

/// How many ended calls to keep for the call list.
const MAX_ENDED: usize = 30;

#[derive(Default)]
pub struct CallRegistry {
    calls: DashMap<String, Arc<CallRecord>>,
}

impl CallRegistry {
    pub fn create(&self, id: &str, from: &str, source: CallSource) -> Arc<CallRecord> {
        self.prune_ended();
        let record = Arc::new(CallRecord::new(id.to_string(), from.to_string(), source));
        self.calls.insert(id.to_string(), Arc::clone(&record));
        record
    }

    pub fn get(&self, id: &str) -> Option<Arc<CallRecord>> {
        self.calls.get(id).map(|entry| Arc::clone(entry.value()))
    }

    /// Newest first.
    pub fn summaries(&self) -> Vec<CallSummary> {
        let mut list: Vec<_> = self.calls.iter().map(|entry| entry.summary()).collect();
        list.sort_by(|a, b| {
            b.started_at_ms
                .cmp(&a.started_at_ms)
                .then_with(|| b.id.cmp(&a.id))
        });
        list
    }

    fn prune_ended(&self) {
        let mut ended: Vec<(u64, String)> = self
            .calls
            .iter()
            .filter(|entry| entry.is_ended())
            .map(|entry| (entry.started_at_ms, entry.id.clone()))
            .collect();
        if ended.len() < MAX_ENDED {
            return;
        }
        ended.sort();
        for (_, id) in &ended[..=ended.len() - MAX_ENDED] {
            self.calls.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calls::CallEvent;

    #[test]
    fn keeps_a_bounded_number_of_ended_calls() {
        let registry = CallRegistry::default();
        for i in 0..MAX_ENDED + 5 {
            registry
                .create(&format!("CA{i}"), "+1", CallSource::Live)
                .publish(CallEvent::Ended);
        }
        let active = registry.create("CA_ACTIVE", "+1", CallSource::Live);
        assert!(registry.summaries().len() <= MAX_ENDED + 1);
        assert!(registry.get("CA_ACTIVE").is_some());
        drop(active);
    }
}
