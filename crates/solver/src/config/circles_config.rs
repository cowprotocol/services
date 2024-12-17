use ethcontract::H160;

pub struct CirclesConfig {
    pub known_hub_addresses: Vec<H160>,
}

impl CirclesConfig {
    pub fn new(known_hubs: Vec<H160>) -> Self {
        Self { known_hub_addresses: known_hubs }
    }

    pub fn is_known_hub(&self, hub_addr: H160) -> bool {
        self.known_hub_addresses.contains(&hub_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_hub() {
        let hub1: H160 = "0x1111111111111111111111111111111111111111".parse().unwrap();
        let hub2: H160 = "0x2222222222222222222222222222222222222222".parse().unwrap();
        let config = CirclesConfig::new(vec![hub1, hub2]);

        assert!(config.is_known_hub(hub1));
        assert!(config.is_known_hub(hub2));
        let unknown: H160 = "0x3333333333333333333333333333333333333333".parse().unwrap();
        assert!(!config.is_known_hub(unknown));
    }
} 