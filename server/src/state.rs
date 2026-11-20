//! Shared application state handed to every handler.

use std::sync::Arc;

use detector::RuleSet;

use crate::{calls::CallRegistry, config::Config, replay::ReplayConfig};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub calls: Arc<CallRegistry>,
    pub rules: Arc<RuleSet>,
    pub replay: Arc<ReplayConfig>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self::with_replay(config, ReplayConfig::default())
    }

    pub fn with_replay(config: Config, replay: ReplayConfig) -> Self {
        Self {
            config: Arc::new(config),
            calls: Arc::new(CallRegistry::default()),
            rules: Arc::new(RuleSet::default_rules()),
            replay: Arc::new(replay),
        }
    }
}
