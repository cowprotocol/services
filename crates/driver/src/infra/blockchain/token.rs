use {
    super::{Error, Ethereum},
    crate::domain::{competition::order, eth},
    ethabi::{ParamType, Token},
    ethcontract::{Account, PrivateKey},
    futures::TryFutureExt,
};

/// An ERC-20 token.
///
/// https://eips.ethereum.org/EIPS/eip-20
pub struct Erc20 {
    token: contracts::ERC20,
    ethereum: Ethereum,
    simulator: Box<dyn TradableBalanceSimulator + Send + Sync>,
}

impl Erc20 {
    pub(super) fn new(eth: &Ethereum, address: eth::TokenAddress) -> Self {
        let chain_id = eth.chain().id();
        let simulator: Box<dyn TradableBalanceSimulator> = match chain_id {
            232 | 324 => Box::new(ZkSyncTradableBalanceSimulator),
            _ => Box::new(EvmTradableBalanceSimulator),
        };

        Self {
            token: eth.contract_at(address.into()),
            ethereum: eth.clone(),
            simulator,
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
        if interactions.is_empty() {
            self.tradable_balance_simple(trader, source).await
        } else {
            self.simulator
                .simulate(
                    &self.ethereum,
                    self.token.address(),
                    trader,
                    source,
                    interactions,
                )
                .await
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

#[async_trait::async_trait]
trait TradableBalanceSimulator: Send + Sync {
    async fn simulate(
        &self,
        ethereum: &Ethereum,
        token: eth::H160,
        trader: eth::Address,
        source: order::SellTokenBalance,
        interactions: &[eth::Interaction],
    ) -> Result<eth::TokenAmount, Error>;
}

struct EvmTradableBalanceSimulator;

#[async_trait::async_trait]
impl TradableBalanceSimulator for EvmTradableBalanceSimulator {
    async fn simulate(
        &self,
        ethereum: &Ethereum,
        token: eth::H160,
        trader: eth::Address,
        source: order::SellTokenBalance,
        interactions: &[eth::Interaction],
    ) -> Result<eth::TokenAmount, Error> {
        let balance_helper = ethereum.contracts().balance_helper();
        let mut method = balance_helper.balance(
            (
                balance_helper.address(),
                ethereum.contracts().vault_relayer().into(),
                ethereum.contracts().vault().address(),
            ),
            trader.into(),
            token,
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
        );
        // Create the access list for the balance simulation
        let access_list_call = contracts::storage_accessible::call(
            method.tx.to.unwrap(),
            contracts::bytecode!(contracts::support::Balances),
            method.tx.data.clone().unwrap(),
        );
        let access_list = ethereum.create_access_list(access_list_call).await.ok();
        method.tx.access_list = access_list.map(Into::into);
        let (_, _, effective_balance, can_transfer) = contracts::storage_accessible::simulate(
            contracts::bytecode!(contracts::support::Balances),
            method,
        )
        .await?;

        if can_transfer {
            Ok(effective_balance.into())
        } else {
            Ok(eth::TokenAmount(0.into()))
        }
    }
}

struct ZkSyncTradableBalanceSimulator;

impl ZkSyncTradableBalanceSimulator {
    // @todo: deduplicate
    fn random_account() -> Account {
        let mut buffer = [0; 32];
        let mut start: usize = 100500;
        loop {
            buffer[24..].copy_from_slice(&start.to_be_bytes());
            let Ok(pk) = PrivateKey::from_raw(buffer) else {
                start += 1;
                continue;
            };

            break Account::Offline(pk, None);
        }
    }
}

#[async_trait::async_trait]
impl TradableBalanceSimulator for ZkSyncTradableBalanceSimulator {
    async fn simulate(
        &self,
        ethereum: &Ethereum,
        token: eth::H160,
        trader: eth::Address,
        source: order::SellTokenBalance,
        interactions: &[eth::Interaction],
    ) -> Result<eth::TokenAmount, Error> {
        let balance_helper = ethereum.contracts().balance_helper();
        let balance_call = balance_helper.balance(
            (
                ethereum.contracts().settlement().address(),
                ethereum.contracts().vault_relayer().into(),
                ethereum.contracts().vault().address(),
            ),
            trader.into(),
            token,
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
        );
        let calldata = balance_call.tx.data.clone();
        let random_account = Self::random_account();
        let method = ethereum
            .contracts()
            .storage_accessible()
            .simulate_delegatecall(
                balance_helper.address(),
                ethcontract::Bytes(balance_call.tx.data.unwrap_or_default().0),
            )
            .from(random_account);
        let tx = method.tx.clone();
        let result = method.call().await;
        // @todo: remove
        if result.is_err() {
            tracing::warn!(
                ?result,
                ?calldata,
                ?tx,
                "newlog simulating tradable balance failed with"
            );
        }
        let result = result?.0;
        // @todo: switch to abi_decode
        let tokens = ethabi::decode(
            &[
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Bool,
            ],
            &result,
        )
        .map_err(|err| {
            // todo: remove
            tracing::warn!("newlog decode error={:?}", err);
            Error::Web3(web3::error::Error::Decoder("decode error".to_string()))
        })?;
        let (effective_balance, can_transfer) = match tokens.as_slice() {
            [
                Token::Uint(_),
                Token::Uint(_),
                Token::Uint(effective_balance),
                Token::Bool(can_transfer),
            ] => (eth::U256::from(effective_balance), *can_transfer),
            _ => {
                tracing::warn!(?tokens, "newlog unexpected decode result");
                return Err(Error::Web3(web3::error::Error::Decoder(
                    "unexpected decode result".to_string(),
                )));
            }
        };

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
