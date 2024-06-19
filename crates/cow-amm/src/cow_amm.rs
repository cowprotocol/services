use ethcontract::Address;

#[derive(Clone)]
pub struct CowAmm {
    address: Address,
    tradeable_pairs: Vec<Address>,
}

impl CowAmm {
    pub fn new(address: Address, tradeable_pairs: &[Address]) -> Self {
        Self {
            address,
            tradeable_pairs: tradeable_pairs.to_vec(),
        }
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
