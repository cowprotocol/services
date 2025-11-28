//! Module containing a general ERC20 allowance manager that allows components
//! and interactions to query allowances to various contracts as well as keep
//! generate interactions for them.

use {
    crate::interactions::Erc20ApproveInteraction,
    alloy::{
        primitives::{Address, U256},
        sol_types::SolCall,
    },
    anyhow::{Context as _, Result, anyhow, ensure},
    contracts::alloy::ERC20,
    ethrpc::{Web3, alloy::conversions::IntoLegacy},
    maplit::hashmap,
    shared::{
        http_solver::model::TokenAmount,
        interaction::{EncodedInteraction, Interaction},
    },
    std::{
        collections::{HashMap, HashSet},
        slice,
    },
    web3::types::CallRequest,
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
    pub amount: alloy::primitives::U256,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Allowances {
    spender: Address,
    allowances: HashMap<Address, alloy::primitives::U256>,
}

impl Allowances {
    pub fn new(spender: Address, allowances: HashMap<Address, alloy::primitives::U256>) -> Self {
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
            amount: alloy::primitives::U256::MAX,
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
            self.web3.clone(),
            self.owner,
            hashmap! { spender => tokens },
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

        let allowances = fetch_allowances(self.web3.clone(), self.owner, spender_tokens).await?;
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

async fn fetch_allowances<T>(
    web3: Web3<T>,
    owner: Address,
    spender_tokens: HashMap<Address, HashSet<Address>>,
) -> Result<HashMap<Address, Allowances>>
where
    T: ethcontract::web3::BatchTransport + Send + Sync + 'static,
    T::Batch: Send,
    T::Out: Send,
{
    let futures = spender_tokens
        .into_iter()
        .flat_map(|(spender, tokens)| tokens.into_iter().map(move |token| (spender, token)))
        .map(|(spender, token)| {
            let web3 = web3.clone();

            async move {
                let calldata = ERC20::ERC20::allowanceCall { owner, spender }.abi_encode();
                let req = CallRequest::builder()
                    .to(token.into_legacy())
                    .data(calldata.into())
                    .build();
                let allowance = web3.eth().call(req, None).await;
                (spender, token, allowance)
            }
        });
    let results: Vec<_> = futures::future::join_all(futures).await;

    let mut allowances = HashMap::new();
    for (spender, token, allowance) in results {
        let allowance = match allowance {
            Ok(allowance) => U256::from_be_slice(&allowance.0),
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
        alloy::sol_types::SolCall,
        ethcontract::{
            common::abi::{self, Token},
            web3::types::CallRequest,
        },
        ethrpc::mock,
        maplit::{hashmap, hashset},
        serde_json::{Value, json},
        shared::addr,
    };

    #[test]
    fn approval_when_allowance_is_sufficient() {
        let token = Address::repeat_byte(0x02);
        let allowances = Allowances::new(
            Address::repeat_byte(0x01),
            hashmap! {
                token => alloy::primitives::U256::from(100),
            },
        );

        assert_eq!(
            allowances
                .approve_token(TokenAmount::new(token, alloy::primitives::U256::from(42)))
                .unwrap(),
            None
        );
        assert_eq!(
            allowances
                .approve_token(TokenAmount::new(token, alloy::primitives::U256::from(100)))
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
                token => alloy::primitives::U256::from(100),
            },
        );

        assert_eq!(
            allowances
                .approve_token(TokenAmount::new(token, alloy::primitives::U256::from(1337)))
                .unwrap(),
            Some(Approval { token, spender })
        );
    }

    #[test]
    fn approval_for_missing_token() {
        let allowances = Allowances::new(
            Address::repeat_byte(0x01),
            hashmap! {
                Address::repeat_byte(0x02) => alloy::primitives::U256::from(100),
            },
        );

        assert!(
            allowances
                .approve_token(TokenAmount::new(
                    Address::repeat_byte(0x03),
                    alloy::primitives::U256::ZERO
                ))
                .is_err()
        );
    }

    #[test]
    fn approval_or_default_for_missing_token() {
        let spender = Address::repeat_byte(0x01);
        let token = Address::repeat_byte(0x02);
        let allowances = Allowances::new(spender, hashmap! {});

        assert_eq!(
            allowances.approve_token_or_default(TokenAmount::new(
                token,
                alloy::primitives::U256::from(1337)
            )),
            Some(Approval { token, spender })
        );
    }

    #[test]
    fn extend_allowances_cache() {
        let mut allowances = Allowances::new(
            Address::repeat_byte(0x01),
            hashmap! {
                Address::repeat_byte(0x11) => alloy::primitives::U256::from(1),
                Address::repeat_byte(0x12) => alloy::primitives::U256::from(2),
            },
        );
        allowances
            .extend(Allowances::new(
                Address::repeat_byte(0x01),
                hashmap! {
                    Address::repeat_byte(0x11) => alloy::primitives::U256::from(42),
                    Address::repeat_byte(0x13) => alloy::primitives::U256::from(3),
                },
            ))
            .unwrap();

        assert_eq!(
            allowances.allowances,
            hashmap! {
                Address::repeat_byte(0x11) => alloy::primitives::U256::from(42),
                Address::repeat_byte(0x12) => alloy::primitives::U256::from(2),
                Address::repeat_byte(0x13) => alloy::primitives::U256::from(3),
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
                alloy::primitives::U256::ZERO,
                const_hex::decode(
                    "095ea7b3\
                    0000000000000000000000000202020202020202020202020202020202020202\
                    ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                )
                .unwrap().into()
            )
        );
    }

    fn allowance_return_data(value: ethcontract::U256) -> Value {
        json!(web3::types::Bytes(abi::encode(&[Token::Uint(value)])))
    }

    #[tokio::test]
    async fn fetch_skips_failed_allowance_calls() {
        let owner = Address::repeat_byte(1);
        let spender = Address::repeat_byte(2);

        let web3 = mock::web3();
        web3.transport()
            .mock()
            .expect_execute()
            .returning(move |method, params| {
                assert_eq!(method, "eth_call");

                let call = serde_json::from_value::<CallRequest>(params[0].clone()).unwrap();
                assert_eq!(
                    call.data.unwrap(),
                    contracts::alloy::ERC20::ERC20::allowanceCall { owner, spender }
                        .abi_encode()
                        .into()
                );
                let to = call.to.unwrap();

                if to == addr!("1111111111111111111111111111111111111111") {
                    Ok(allowance_return_data(1337.into()))
                } else if to == addr!("2222222222222222222222222222222222222222") {
                    Err(web3::Error::Decoder("test error".to_string()))
                } else {
                    panic!("call to unexpected token {to:?}")
                }
            });

        let allowances = fetch_allowances(
            web3,
            owner,
            hashmap! {
                spender => hashset![Address::repeat_byte(0x11), Address::repeat_byte(0x22)],
            },
        )
        .await
        .unwrap();

        assert_eq!(
            allowances,
            hashmap! {
                spender => Allowances {
                    spender,
                    allowances: hashmap! { Address::repeat_byte(0x11) => alloy::primitives::U256::from(1337) },
                },
            },
        );
    }
}
