impl TryFrom<(web3::types::Transaction, web3::types::TransactionReceipt)>
    for crate::domain::settlement::Transaction
{
    type Error = anyhow::Error;

    fn try_from(
        (transaction, receipt): (web3::types::Transaction, web3::types::TransactionReceipt),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: transaction.hash.into(),
            solver: transaction
                .from
                .ok_or(anyhow::anyhow!("missing from"))?
                .into(),
            input: crate::util::Bytes(transaction.input.0),
            block: receipt
                .block_number
                .ok_or(anyhow::anyhow!("missing block_number"))?
                .0[0]
                .into(),
            gas: receipt
                .gas_used
                .ok_or(anyhow::anyhow!("missing gas_used"))?,
            effective_gas_price: receipt
                .effective_gas_price
                .ok_or(anyhow::anyhow!("missing effective_gas_price"))?,
        })
    }
}
