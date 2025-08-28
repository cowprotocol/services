use {
    super::{Error, Ethereum},
    crate::domain::{competition::order, eth},
    tap::TapFallible,
    web3::types::CallRequest,
};

/// An ERC-20 token.
///
/// https://eips.ethereum.org/EIPS/eip-20
pub struct Erc20 {
    token: contracts::ERC20,
    ethereum: Ethereum,
}

impl Erc20 {
    pub(super) fn new(eth: &Ethereum, address: eth::TokenAddress) -> Self {
        Self {
            token: eth.contract_at(address.into()),
            ethereum: eth.clone(),
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
        disable_access_list_simulation: bool,
    ) -> Result<eth::TokenAmount, Error> {
        self.tradable_balance_simulated(
            trader,
            source,
            interactions,
            disable_access_list_simulation,
        )
        .await
    }

    /// Uses a custom helper contract to simulate balances while taking
    /// pre-interactions into account. This is the most accurate method to
    /// compute tradable balances but is very slow.
    async fn tradable_balance_simulated(
        &self,
        trader: eth::Address,
        source: order::SellTokenBalance,
        interactions: &[eth::Interaction],
        disable_access_lists: bool,
    ) -> Result<eth::TokenAmount, Error> {
        let interactions: Vec<_> = interactions.iter().map(|i| i.clone().into()).collect();
        let simulation = self
            .ethereum
            .balance_simulator()
            .simulate(
                trader.0,
                self.token.address(),
                source.into(),
                &interactions,
                None,
                |mut delegate_call| {
                    let ethereum = self.ethereum.clone();
                    async move {
                        // Add the access lists to the delegate call if they are enabled
                        // system-wide.
                        if disable_access_lists {
                            return delegate_call;
                        }

                        let access_list_call = CallRequest {
                            data: delegate_call.tx.data.clone(),
                            from: delegate_call.tx.from.clone().map(|acc| acc.address()),
                            ..Default::default()
                        };
                        let access_list = ethereum
                            .create_access_list(access_list_call)
                            .await
                            .tap_err(|err| {
                                tracing::debug!(
                                    ?err,
                                    "failed to create access list for balance simulation"
                                );
                            })
                            .ok();
                        delegate_call.tx.access_list = access_list.map(Into::into);
                        delegate_call
                    }
                },
            )
            .await?;

        Ok(simulation.effective_balance.into())
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
