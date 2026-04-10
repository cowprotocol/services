#[derive(Debug, Clone, Copy)]
pub struct EstimatorConfig {
    /// Number of blocks to look back for fee history
    pub past_blocks: u64,
    /// Percentile of rewards to use for priority fee estimation
    pub reward_percentile: f64,
}

pub fn default_past_blocks() -> u64 {
    10
}

pub fn default_reward_percentile() -> f64 {
    20.0
}
