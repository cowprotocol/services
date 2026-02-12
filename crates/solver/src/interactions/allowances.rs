//! Module containing a general ERC20 allowance manager that allows components
//! and interactions to query allowances to various contracts as well as keep
//! generate interactions for them.

use {
    crate::interactions::Erc20ApproveInteraction,
    alloy::{
        network::Ethereum,
        primitives::{Address, U256},
        providers::DynProvider,
    },
    anyhow::{Context as _, Result, anyhow, ensure},
    contracts::alloy::ERC20,
    ethrpc::Web3,
    shared::{
        http_solver::model::TokenAmount,
        interaction::{EncodedInteraction, Interaction},
    },
    std::{
        collections::{HashMap, HashSet},
        slice,
    },
    tracing::instrument,
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait AllowanceManaging: Send + Sync {
    /// Retrieves allowances of the specified tokens for a given spender.
    ///
    /// This can be used to cache allowances for a bunch of tokens so that they
    /// can be used within a context that doesn't allow `async` or errors.
    async fn get_allowances(
        &self,
        tokens: HashSet<Address>,
        spender: Address,
    ) -> Result<Allowances>;

    /// Returns the approval interaction for the specified token and spender for
    /// at least the specified amount, if an approval is required.
    async fn get_approval(&self, request: &ApprovalRequest) -> Result<Option<Approval>> {
        Ok(self.get_approvals(slice::from_ref(request)).await?.pop())
    }

    /// Returns the requried approval interaction for the requests.
    /// Does not return approvals when they aren't required.
    async fn get_approvals(&self, requests: &[ApprovalRequest]) -> Result<Vec<Approval>>;
}

#[derive(Debug, Eq, PartialEq)]
pub struct ApprovalRequest {
    pub token: Address,
    pub spender: Address,
    pub amount: U256,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Allowances {
    spender: Address,
    allowances: HashMap<Address, U256>,
}

impl Allowances {
    pub fn new(spender: Address, allowances: HashMap<Address, U256>) -> Self {
        Self {
            spender,
            allowances,
        }
    }

    pub fn empty(spender: Address) -> Self {
        Self::new(spender, HashMap::new())
    }

    /// Gets the approval interaction for the specified token and amount.
    pub fn approve_token(&self, token_amount: TokenAmount) -> Result<Option<Approval>> {
        let allowance = self
            .allowances
            .get(&token_amount.token)
            .copied()
            .ok_or_else(|| anyhow!("missing allowance for token {:?}", token_amount.token))?;

        Ok(if allowance < token_amount.amount {
            Some(Approval {
                token: token_amount.token,
                spender: self.spender,
            })
        } else {
            None
        })
    }

    /// Gets the token approval, unconditionally approving in case the token
    /// allowance is missing.
    pub fn approve_token_or_default(&self, token_amount: TokenAmount) -> Option<Approval> {
        let token = token_amount.token;

        self.approve_token(token_amount).unwrap_or(Some(Approval {
            token,
            spender: self.spender,
        }))
    }

    /// Extends the allowance cache with another.
    pub fn extend(&mut self, other: Self) -> Result<()> {
        ensure!(
            self.spender == other.spender,
            "failed to extend allowance cache for different spender"
        );
        self.allowances.extend(other.allowances);

        Ok(())
    }
}

/// An ERC20 approval interaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Approval {
    pub token: Address,
    pub spender: Address,
}

impl Interaction for Approval {
    fn encode(&self) -> EncodedInteraction {
        let approve = Erc20ApproveInteraction {
            token: self.token,
            spender: self.spender,
            amount: U256::MAX,
        };

        approve.encode()
    }
}

/// An allowance manager that retrive approval interactions for a given owner
/// address.
pub struct AllowanceManager {
    web3: Web3,
    owner: Address,
}

impl AllowanceManager {
    pub fn new(web3: Web3, owner: Address) -> Self {
        Self { web3, owner }
    }
}

#[async_trait::async_trait]
impl AllowanceManaging for AllowanceManager {
    async fn get_allowances(
        &self,
        tokens: HashSet<Address>,
        spender: Address,
    ) -> Result<Allowances> {
        Ok(fetch_allowances(
            self.web3.provider.clone(),
            self.owner,
            HashMap::from([(spender, tokens)]),
        )
        .await?
        .remove(&spender)
        .unwrap_or_else(|| Allowances::empty(spender)))
    }

    async fn get_approvals(&self, requests: &[ApprovalRequest]) -> Result<Vec<Approval>> {
        let mut spender_tokens = HashMap::<_, HashSet<_>>::new();
        for request in requests {
            spender_tokens
                .entry(request.spender)
                .or_default()
                .insert(request.token);
        }

        let allowances =
            fetch_allowances(self.web3.provider.clone(), self.owner, spender_tokens).await?;
        let mut result = Vec::new();
        for request in requests {
            let allowance = allowances
                .get(&request.spender)
                .with_context(|| format!("no allowances found for spender {}", request.spender))?
                .approve_token(TokenAmount::new(request.token, request.amount))?;
            result.extend(allowance);
        }
        Ok(result)
    }
}

#[instrument(skip_all)]
async fn fetch_allowances(
    alloy: DynProvider<Ethereum>,
    owner: Address,
    spender_tokens: HashMap<Address, HashSet<Address>>,
) -> Result<HashMap<Address, Allowances>> {
    let futures = spender_tokens
        .into_iter()
        .flat_map(|(spender, tokens)| tokens.into_iter().map(move |token| (spender, token)))
        .map(|(spender, token)| {
            let alloy = alloy.clone();
            async move {
                let allowance = ERC20::Instance::new(token, alloy)
                    .allowance(owner, spender)
                    .call()
                    .await;

                (spender, token, allowance)
            }
        });
    let results: Vec<_> = futures::future::join_all(futures).await;

    let mut allowances = HashMap::new();
    for (spender, token, allowance) in results {
        let allowance = match allowance {
            Ok(allowance) => allowance,
            Err(err) => {
                tracing::warn!("error retrieving allowance for token {:?}: {}", token, err);
                continue;
            }
        };

        allowances
            .entry(spender)
            .or_insert_with(|| Allowances::empty(spender))
            .allowances
            .insert(token, allowance);
    }

    Ok(allowances)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{
            providers::{Provider, ProviderBuilder, mock::Asserter},
            sol_types::SolValue,
        },
        maplit::{hashmap, hashset},
    };

    #[test]
    fn approval_when_allowance_is_sufficient() {
        let token = Address::repeat_byte(0x02);
        let allowances = Allowances::new(
            Address::repeat_byte(0x01),
            hashmap! {
                token => U256::from(100),
            },
        );

        assert_eq!(
            allowances
                .approve_token(TokenAmount::new(token, U256::from(42)))
                .unwrap(),
            None
        );
        assert_eq!(
            allowances
                .approve_token(TokenAmount::new(token, U256::from(100)))
                .unwrap(),
            None
        );
    }

    #[test]
    fn approval_when_allowance_is_insufficient() {
        let spender = Address::repeat_byte(0x01);
        let token = Address::repeat_byte(0x02);
        let allowances = Allowances::new(
            spender,
            hashmap! {
                token => U256::from(100),
            },
        );

        assert_eq!(
            allowances
                .approve_token(TokenAmount::new(token, U256::from(1337)))
                .unwrap(),
            Some(Approval { token, spender })
        );
    }

    #[test]
    fn approval_for_missing_token() {
        let allowances = Allowances::new(
            Address::repeat_byte(0x01),
            hashmap! {
                Address::repeat_byte(0x02) => U256::from(100),
            },
        );

        assert!(
            allowances
                .approve_token(TokenAmount::new(Address::repeat_byte(0x03), U256::ZERO))
                .is_err()
        );
    }

    #[test]
    fn approval_or_default_for_missing_token() {
        let spender = Address::repeat_byte(0x01);
        let token = Address::repeat_byte(0x02);
        let allowances = Allowances::new(spender, hashmap! {});

        assert_eq!(
            allowances.approve_token_or_default(TokenAmount::new(token, U256::from(1337))),
            Some(Approval { token, spender })
        );
    }

    #[test]
    fn extend_allowances_cache() {
        let mut allowances = Allowances::new(
            Address::repeat_byte(0x01),
            hashmap! {
                Address::repeat_byte(0x11) => U256::from(1),
                Address::repeat_byte(0x12) => U256::from(2),
            },
        );
        allowances
            .extend(Allowances::new(
                Address::repeat_byte(0x01),
                hashmap! {
                    Address::repeat_byte(0x11) => U256::from(42),
                    Address::repeat_byte(0x13) => U256::from(3),
                },
            ))
            .unwrap();

        assert_eq!(
            allowances.allowances,
            hashmap! {
                Address::repeat_byte(0x11) => U256::from(42),
                Address::repeat_byte(0x12) => U256::from(2),
                Address::repeat_byte(0x13) => U256::from(3),
            },
        );
    }

    #[test]
    fn error_extending_allowances_for_different_spenders() {
        let mut allowances = Allowances::empty(Address::repeat_byte(0x01));
        assert!(
            allowances
                .extend(Allowances::empty(Address::repeat_byte(0x02)))
                .is_err()
        );
    }

    #[test]
    fn approval_encode_interaction() {
        let token = Address::repeat_byte(0x01);
        let spender = Address::repeat_byte(0x02);
        assert_eq!(
            Approval { token, spender }.encode(),
            (
                token,
                U256::ZERO,
                const_hex::decode(
                    "095ea7b3\
                    0000000000000000000000000202020202020202020202020202020202020202\
                    ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                )
                .unwrap().into()
            )
        );
    }

    #[tokio::test]
    async fn fetch_skips_failed_allowance_calls() {
        let owner = Address::repeat_byte(1);
        let spender = Address::repeat_byte(2);

        let asserter = Asserter::new();
        let provider = ProviderBuilder::new().connect_mocked_client(asserter.clone());

        asserter.push_success(&U256::from(1337).abi_encode());
        asserter.push_failure_msg("test error");

        let allowances = fetch_allowances(
            provider.erased(),
            owner,
            hashmap! {
                spender => hashset![Address::repeat_byte(0x11), Address::repeat_byte(0x22)],
            },
        )
        .await
        .unwrap();

        // Poor man's assert + get
        let allowances = allowances
            .get(&spender)
            .unwrap_or_else(|| panic!("should have a spender key {:?}", spender));
        assert_eq!(allowances.spender, spender);
        // We don't check which of the two (0x11, 0x22) got the error vs the result
        // because it's order dependent and without a custom mock transport just for
        // this test, we wouldn't be able to make it deterministic
        assert_eq!(allowances.allowances.len(), 1);
        assert_eq!(
            allowances.allowances.values().next(),
            Some(&U256::from(1337))
        );
    }
}
