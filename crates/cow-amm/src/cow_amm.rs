use ethcontract::{Address, Bytes};

#[derive(Clone)]
pub struct CowAmm {
    address: Address,
    tradeable_pairs: Vec<Address>,
    // This is a placeholder for the actual CoW AMM arbitrary bytes obtained from tradingEnabled
    // (`TradingParams`).
    bytes: Bytes<[u8; 32]>,
}

impl CowAmm {
    pub fn new(address: Address, tradeable_pairs: &[Address]) -> Self {
        Self {
            address,
            tradeable_pairs: tradeable_pairs.to_vec(),
            bytes: Bytes::default(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.bytes != Bytes::default()
    }

    pub fn disable(&mut self) {
        self.bytes = Bytes::default();
    }

    pub fn set_bytes(&mut self, bytes: Bytes<[u8; 32]>) {
        self.bytes = bytes;
    }
}

impl crate::CowAmm for CowAmm {
    fn address(&self) -> &Address {
        &self.address
    }

    fn traded_tokens(&self) -> &[Address] {
        self.tradeable_pairs.as_slice()
    }
}
