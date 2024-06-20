use {ethcontract::Address, std::sync::Arc};

#[derive(Clone)]
pub(crate) struct CowAmm {
    address: Address,
    tradeable_tokens: Vec<Address>,
}

impl CowAmm {
    pub(crate) fn build(address: Address, tradeable_pairs: &[Address]) -> Arc<dyn crate::CowAmm> {
        Arc::new(Self {
            address,
            tradeable_tokens: tradeable_pairs.to_vec(),
        })
    }
}

impl crate::CowAmm for CowAmm {
    fn address(&self) -> &Address {
        &self.address
    }

    fn traded_tokens(&self) -> &[Address] {
        self.tradeable_tokens.as_slice()
    }
}
