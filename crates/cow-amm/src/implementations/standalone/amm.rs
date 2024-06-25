use ethcontract::Address;

#[derive(Clone)]
pub(crate) struct Amm {
    address: Address,
    tradeable_tokens: [Address; 2],
}

impl Amm {
    pub(crate) fn new(address: Address, tradeable_tokens: [Address; 2]) -> Self {
        Self {
            address,
            tradeable_tokens,
        }
    }
}

impl crate::CowAmm for Amm {
    fn address(&self) -> &Address {
        &self.address
    }

    fn traded_tokens(&self) -> &[Address] {
        &self.tradeable_tokens
    }
}
