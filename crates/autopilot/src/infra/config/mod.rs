use chrono::Duration;

#[derive(Debug, Default)]
pub struct Config {
    pub block_time: Duration,
    // Auction lasts up to `auction_block_size` blocks.
    pub auction_blocks_duration: usize,
    // rest of the config
}
