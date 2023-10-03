use {
    super::{Error, Ethereum},
    crate::domain::{competition::order, eth},
};

/// An ERC-20 token.
///
/// https://eips.ethereum.org/EIPS/eip-20
pub struct Erc20 {
    token: contracts::ERC20,
    balances: contracts::support::Balances,
    vault_relayer: eth::ContractAddress,
    vault: eth::ContractAddress,
}

impl Erc20 {
    pub(super) fn new(eth: &Ethereum, address: eth::TokenAddress) -> Self {
        let settlement = eth.contracts().settlement().address().into();
        Self {
            token: eth.contract_at(address.into()),
            balances: eth.contract_at(settlement),
            vault_relayer: eth.contracts().vault_relayer(),
            vault: eth.contracts().vault().address().into(),
        }
    }

    /// Returns the [`eth::TokenAddress`] of the ERC20.
    pub fn address(&self) -> eth::TokenAddress {
        self.token.address().into()
    }

    /// Fetch the ERC20 allowance for the spender. See the allowance method in
    /// EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#allowance
    pub async fn allowance(
        &self,
        owner: eth::Address,
        spender: eth::Address,
    ) -> Result<eth::allowance::Existing, Error> {
        let amount = self.token.allowance(owner.0, spender.0).call().await?;
        Ok(eth::Allowance {
            token: self.token.address().into(),
            spender,
            amount,
        }
        .into())
    }

    /// Fetch the ERC20 token decimals. Returns `None` if the token does not
    /// implement this optional method. See the decimals method in EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#decimals
    pub async fn decimals(&self) -> Result<Option<u8>, Error> {
        match self.token.decimals().call().await {
            Ok(decimals) => Ok(Some(decimals)),
            Err(err) if is_contract_error(&err) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Fetch the ERC20 token symbol. Returns `None` if the token does not
    /// implement this optional method. See the symbol method in EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#symbol
    pub async fn symbol(&self) -> Result<Option<String>, Error> {
        match self.token.symbol().call().await {
            Ok(symbol) => Ok(Some(symbol)),
            Err(err) if is_contract_error(&err) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Fetch the ERC20 balance of the specified account. Returns the current
    /// balance as an [`eth::TokenAmount`]. See the balanceOf method in EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#balanceof
    pub async fn balance(&self, holder: eth::Address) -> Result<eth::TokenAmount, Error> {
        self.token
            .balance_of(holder.0)
            .call()
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Fetches the tradable balance for the specified user given an order's
    /// pre-interactions.
    pub async fn tradable_balance(
        &self,
        trader: eth::Address,
        source: order::SellTokenBalance,
        interactions: &[eth::Interaction],
    ) -> Result<eth::TokenAmount, Error> {
        let (_, _, effective_balance, can_transfer) = contracts::storage_accessible::simulate(
            contracts::bytecode!(contracts::support::Balances),
            self.balances.balance(
                (
                    self.balances.address(),
                    self.vault_relayer.into(),
                    self.vault.into(),
                ),
                trader.into(),
                self.token.address(),
                0.into(),
                ethcontract::Bytes(source.hash().0),
                interactions
                    .iter()
                    .map(|i| {
                        (
                            i.target.into(),
                            i.value.into(),
                            ethcontract::Bytes(i.call_data.0.clone()),
                        )
                    })
                    .collect(),
            ),
        )
        .await?;

        if can_transfer {
            Ok(effective_balance.into())
        } else {
            Ok(eth::TokenAmount(0.into()))
        }
    }
}

/// Returns `true` if a [`ethcontract::errors::MethodError`] is the result of
/// some on-chain computation error.
fn is_contract_error(err: &ethcontract::errors::MethodError) -> bool {
    // Assume that any error that isn't a `Web3` error is a "contract error",
    // this can mean things like:
    // - The contract call reverted
    // - The returndata cannot be decoded
    // - etc.
    !matches!(&err.inner, ethcontract::errors::ExecutionError::Web3(_))
}
