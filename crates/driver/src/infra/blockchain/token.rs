use {
    super::{Error, Ethereum},
    crate::domain::eth,
    ethrpc::alloy::{
        conversions::{IntoAlloy, IntoLegacy},
        errors::ContractErrorExt,
    },
};

/// An ERC-20 token.
///
/// https://eips.ethereum.org/EIPS/eip-20
pub struct Erc20 {
    token: contracts::alloy::ERC20::Instance,
}

impl Erc20 {
    pub(super) fn new(eth: &Ethereum, address: eth::TokenAddress) -> Self {
        Self {
            token: contracts::alloy::ERC20::Instance::new(
                address.0.0.into_alloy(),
                eth.web3.alloy.clone(),
            ),
        }
    }

    /// Returns the [`eth::TokenAddress`] of the ERC20.
    pub fn address(&self) -> eth::TokenAddress {
        self.token.address().into_legacy().into()
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
        let amount = self
            .token
            .allowance(owner.0.into_alloy(), spender.0.into_alloy())
            .call()
            .await?;
        Ok(eth::Allowance {
            token: self.token.address().into_legacy().into(),
            spender,
            amount: amount.into_legacy(),
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
            Err(err) if err.is_node_error() => Err(err.into()),
            Err(_) => Ok(None),
        }
    }

    /// Fetch the ERC20 token symbol. Returns `None` if the token does not
    /// implement this optional method. See the symbol method in EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#symbol
    pub async fn symbol(&self) -> Result<Option<String>, Error> {
        match self.token.symbol().call().await {
            Ok(symbol) => Ok(Some(symbol)),
            Err(err) if err.is_node_error() => Err(err.into()),
            Err(_) => Ok(None),
        }
    }

    /// Fetch the ERC20 balance of the specified account. Returns the current
    /// balance as an [`eth::TokenAmount`]. See the balanceOf method in EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#balanceof
    pub async fn balance(&self, holder: eth::Address) -> Result<eth::TokenAmount, Error> {
        self.token
            .balanceOf(holder.0.into_alloy())
            .call()
            .await
            .map(IntoLegacy::into_legacy)
            .map(Into::into)
            .map_err(Into::into)
    }
}
