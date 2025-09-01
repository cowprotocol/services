use {
    super::{Error, Ethereum},
    crate::domain::{competition::order, eth},
    futures::TryFutureExt,
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
        if interactions.is_empty() {
            self.tradable_balance_simple(trader, source).await
        } else {
            self.tradable_balance_simulated(
                trader,
                source,
                interactions,
                disable_access_list_simulation,
            )
            .await
        }
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
        let shared::account_balances::Simulation {
            token_balance: _,
            allowance: _,
            effective_balance,
            can_transfer,
        } = self
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
                            // `from` field is not populated, since it is only required for ZkSync
                            // chains, that currently don't support access lists. On other chains,
                            // access lists creation fails when an arbitrary `from` is used.
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

        if can_transfer {
            Ok(effective_balance.into())
        } else {
            Ok(eth::TokenAmount(0.into()))
        }
    }

    /// Faster balance query that does not take pre-interactions into account.
    async fn tradable_balance_simple(
        &self,
        trader: eth::Address,
        source: order::SellTokenBalance,
    ) -> Result<eth::TokenAmount, Error> {
        use order::SellTokenBalance;

        let relayer = self.ethereum.contracts().vault_relayer();
        let usable_balance = match source {
            SellTokenBalance::Erc20 => {
                let balance = self.balance(trader);
                let allowance = self.allowance(trader, eth::Address(relayer.into()));
                let (balance, allowance) = futures::try_join!(balance, allowance)?;
                std::cmp::min(balance.0, allowance.0.amount)
            }
            SellTokenBalance::External => {
                let vault = self.ethereum.contracts().vault();
                let balance = self.balance(trader);
                let approved = vault
                    .methods()
                    .has_approved_relayer(trader.0, relayer.into())
                    .call()
                    .map_err(Error::from);
                let allowance = self.allowance(trader, vault.address().into());
                let (balance, approved, allowance) =
                    futures::try_join!(balance, approved, allowance)?;
                match approved {
                    true => std::cmp::min(balance.0, allowance.0.amount),
                    false => 0.into(),
                }
            }
            SellTokenBalance::Internal => {
                let vault = self.ethereum.contracts().vault();
                let balance = vault
                    .methods()
                    .get_internal_balance(trader.0, vec![self.token.address()])
                    .call()
                    .map_err(Error::from);
                let approved = vault
                    .methods()
                    .has_approved_relayer(trader.0, relayer.into())
                    .call()
                    .map_err(Error::from);
                let (balance, approved) = futures::try_join!(balance, approved)?;
                match approved {
                    true => balance[0], // internal approvals are always U256::MAX
                    false => 0.into(),
                }
            }
        };
        Ok(eth::TokenAmount(usable_balance))
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
