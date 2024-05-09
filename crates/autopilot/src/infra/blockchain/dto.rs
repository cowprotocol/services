impl TryFrom<web3::types::Transaction> for crate::domain::settlement::Transaction {
    type Error = &'static str;

    fn try_from(value: web3::types::Transaction) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: value.hash.into(),
            solver: value.from.ok_or("from")?.into(),
            input: crate::domain::settlement::transaction::CallData(value.input.0.into()),
        })
    }
}

impl TryFrom<web3::types::TransactionReceipt> for crate::domain::settlement::transaction::Receipt {
    type Error = &'static str;

    fn try_from(value: web3::types::TransactionReceipt) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: value.transaction_hash.into(),
            block: value.block_number.ok_or("block_number")?.0[0].into(),
            gas: value.gas_used.ok_or("gas_used")?,
            effective_gas_price: value.effective_gas_price.ok_or("effective_gas_price")?,
        })
    }
}
