use {crate::infra, futures::StreamExt};

pub async fn runloop(eth: &infra::Ethereum, _config: &infra::config::Config) -> ! {
    // use config to determine the runloop's time intervals
    // time intervals need to be split between the different tasks
    // 1. Buidl auction
    // 2. Solving time
    // 3. Settling time
    // 4. Small buffer time

    let mut block_stream = ethrpc::current_block::into_stream(eth.current_block().clone());
    while let Some(_block) = block_stream.next().await {
        // replace true with logic: should auction be executed in this block?
        if true {
            // auction can outlive 1 block time
            tokio::spawn(auction());
        }
    }

    panic!("block stream ended unexpectedly");
}

async fn auction() {}
