use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Score boundaries: Low below `medium`, Medium below `high`, High at or above `high`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Thresholds {
    pub medium: u32,
    pub high: u32,
}

impl Thresholds {
    pub fn level(&self, score: u32) -> RiskLevel {
        if score >= self.high {
            RiskLevel::High
        } else if score >= self.medium {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}

/// The call's current risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Risk {
    pub level: RiskLevel,
    pub score: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_thresholds() {
        let t = Thresholds { medium: 3, high: 6 };
        let levels: Vec<_> = (0..8).map(|s| t.level(s)).collect();
        use RiskLevel::*;
        assert_eq!(levels, [Low, Low, Low, Medium, Medium, Medium, High, High]);
    }
}
