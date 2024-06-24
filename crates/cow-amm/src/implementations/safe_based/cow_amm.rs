use ethcontract::Address;

#[derive(Clone)]
pub(crate) struct CowAmm {
    address: Address,
    tradeable_tokens: [Address; 2],
}

impl CowAmm {
    pub(crate) fn new(address: Address, tradeable_tokens: [Address; 2]) -> Self {
        Self {
            address,
            tradeable_tokens,
        }
    }
}

impl crate::CowAmm for CowAmm {
    fn address(&self) -> &Address {
        &self.address
    }

    fn traded_tokens(&self) -> &[Address; 2] {
        &self.tradeable_tokens
    }
}
