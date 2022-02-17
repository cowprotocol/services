//! Module containing a general ERC20 allowance manager that allows components
//! and interactions to query allowances to various contracts as well as keep
//! generate interactions for them.

use crate::{
    encoding::EncodedInteraction, interactions::Erc20ApproveInteraction, settlement::Interaction,
};
use anyhow::{anyhow, bail, ensure, Result};
use contracts::ERC20;
use ethcontract::{batch::CallBatch, errors::ExecutionError, H160, U256};
use maplit::hashset;
use shared::{dummy_contract, Web3};
use std::collections::{HashMap, HashSet};
use web3::error::TransportError;

const MAX_BATCH_SIZE: usize = 100;
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait AllowanceManaging: Send + Sync {
    /// Retrieves allowances of the specified tokens for a given spender.
    ///
    /// This can be used to cache allowances for a bunch of tokens so that they
    /// can be used within a context that doesn't allow `async` or errors.
    async fn get_allowances(&self, tokens: HashSet<H160>, spender: H160) -> Result<Allowances>;

    /// Returns the approval interaction for the specified token and spender for
    /// at least the specified amount.
    async fn get_approval(&self, token: H160, spender: H160, amount: U256) -> Result<Approval>;
}

pub struct Allowances {
    spender: H160,
    allowances: HashMap<H160, U256>,
}

impl Allowances {
    pub fn new(spender: H160, allowances: HashMap<H160, U256>) -> Self {
        Self {
            spender,
            allowances,
        }
    }

    pub fn empty(spender: H160) -> Self {
        Self::new(spender, HashMap::new())
    }

    /// Gets the approval interaction for the specified token and amount.
    pub fn approve_token(&self, token: H160, amount: U256) -> Result<Approval> {
        let allowance = self
            .allowances
            .get(&token)
            .copied()
            .ok_or_else(|| anyhow!("missing allowance for token {:?}", token))?;

        Ok(if allowance < amount {
            Approval::Approve {
                token,
                spender: self.spender,
            }
        } else {
            Approval::AllowanceSufficient
        })
    }

    /// Gets the token approval, unconditionally approving in case the token
    /// allowance is missing.
    pub fn approve_token_or_default(&self, token: H160, amount: U256) -> Approval {
        self.approve_token(token, amount)
            .unwrap_or(Approval::Approve {
                token,
                spender: self.spender,
            })
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Approval {
    /// The existing allowance is sufficient, so no additional `approve` is required.
    AllowanceSufficient,

    /// An ERC20 approve is needed. This interaction always approves U256::MAX
    /// in order to save gas by allowing approvals to be used over multiple
    /// settlements.
    Approve { token: H160, spender: H160 },
}

impl Interaction for Approval {
    fn encode(&self) -> Vec<EncodedInteraction> {
        match self {
            Approval::AllowanceSufficient => vec![],
            Approval::Approve { token, spender } => {
                // Use a "dummy" contract - unfortunately `ethcontract` doesn't
                // allow you use the generated contract intances to encode
                // transaction data without a `Web3` instance. Hopefully, this
                // limitation will be lifted soon to clean up stuff like this.
                let token = dummy_contract!(ERC20, *token);
                let approve = Erc20ApproveInteraction {
                    token,
                    spender: *spender,
                    amount: U256::max_value(),
                };

                approve.encode()
            }
        }
    }
}

/// An allowance manager that retrive approval interactions for a given owner
/// address.
pub struct AllowanceManager {
    web3: Web3,
    owner: H160,
}

impl AllowanceManager {
    pub fn new(web3: Web3, owner: H160) -> Self {
        Self { web3, owner }
    }
}

#[async_trait::async_trait]
impl AllowanceManaging for AllowanceManager {
    async fn get_allowances(&self, tokens: HashSet<H160>, spender: H160) -> Result<Allowances> {
        Ok(Allowances::new(
            spender,
            fetch_allowances(self.web3.clone(), tokens, self.owner, spender).await?,
        ))
    }

    async fn get_approval(&self, token: H160, spender: H160, amount: U256) -> Result<Approval> {
        self.get_allowances(hashset![token], spender)
            .await?
            .approve_token(token, amount)
    }
}

async fn fetch_allowances<T>(
    web3: ethcontract::Web3<T>,
    tokens: HashSet<H160>,
    owner: H160,
    spender: H160,
) -> Result<HashMap<H160, U256>>
where
    T: ethcontract::web3::BatchTransport + Send + Sync + 'static,
    T::Batch: Send,
    T::Out: Send,
{
    let mut batch = CallBatch::new(web3.transport());
    let results: Vec<_> = tokens
        .into_iter()
        .map(|token| {
            let allowance = ERC20::at(&web3, token)
                .allowance(owner, spender)
                .batch_call(&mut batch);
            (token, allowance)
        })
        .collect();

    batch.execute_all(MAX_BATCH_SIZE).await;

    let mut allowances = HashMap::new();
    for (token, allowance) in results {
        let allowance = match allowance.await {
            Ok(value) => value,
            Err(err) if is_batch_error(&err.inner) => bail!(err),
            Err(err) => {
                tracing::warn!("error retrieving allowance for token {:?}: {}", token, err);
                continue;
            }
        };
        allowances.insert(token, allowance);
    }

    Ok(allowances)
}

fn is_batch_error(err: &ExecutionError) -> bool {
    match &err {
        ExecutionError::Web3(web3::Error::Transport(TransportError::Message(message))) => {
            // Currently, there is no sure-fire way to determine if a Web3 error
            // is caused because of a failing batch request, or some a call
            // specific error, so we test that the method starts with "Batch"
            // string as a best guess.
            // <https://github.com/gnosis/ethcontract-rs/issues/550>
            message.starts_with("Batch")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::{
        common::abi::{self, Token},
        web3::types::CallRequest,
        Bytes,
    };
    use maplit::hashmap;
    use serde_json::{json, Value};
    use shared::{addr, transport::mock};

    #[test]
    fn approval_when_allowance_is_sufficient() {
        let token = H160([0x02; 20]);
        let allowances = Allowances::new(
            H160([0x01; 20]),
            hashmap! {
                token => U256::from(100),
            },
        );

        assert_eq!(
            allowances.approve_token(token, 42.into()).unwrap(),
            Approval::AllowanceSufficient
        );
        assert_eq!(
            allowances.approve_token(token, 100.into()).unwrap(),
            Approval::AllowanceSufficient
        );
    }

    #[test]
    fn approval_when_allowance_is_insufficient() {
        let spender = H160([0x01; 20]);
        let token = H160([0x02; 20]);
        let allowances = Allowances::new(
            spender,
            hashmap! {
                token => U256::from(100),
            },
        );

        assert_eq!(
            allowances.approve_token(token, 1337.into()).unwrap(),
            Approval::Approve { token, spender }
        );
    }

    #[test]
    fn approval_for_missing_token() {
        let allowances = Allowances::new(
            H160([0x01; 20]),
            hashmap! {
                H160([0x02; 20]) => U256::from(100),
            },
        );

        assert!(allowances
            .approve_token(H160([0x03; 20]), 0.into())
            .is_err());
    }

    #[test]
    fn approval_or_default_for_missing_token() {
        let spender = H160([0x01; 20]);
        let token = H160([0x02; 20]);
        let allowances = Allowances::new(spender, hashmap! {});

        assert_eq!(
            allowances.approve_token_or_default(token, 1337.into()),
            Approval::Approve { token, spender }
        );
    }

    #[test]
    fn extend_allowances_cache() {
        let mut allowances = Allowances::new(
            H160([0x01; 20]),
            hashmap! {
                H160([0x11; 20]) => U256::from(1),
                H160([0x12; 20]) => U256::from(2),
            },
        );
        allowances
            .extend(Allowances::new(
                H160([0x01; 20]),
                hashmap! {
                    H160([0x11; 20]) => U256::from(42),
                    H160([0x13; 20]) => U256::from(3),
                },
            ))
            .unwrap();

        assert_eq!(
            allowances.allowances,
            hashmap! {
                H160([0x11; 20]) => U256::from(42),
                H160([0x12; 20]) => U256::from(2),
                H160([0x13; 20]) => U256::from(3),
            },
        );
    }

    #[test]
    fn error_extending_allowances_for_different_spenders() {
        let mut allowances = Allowances::empty(H160([0x01; 20]));
        assert!(allowances
            .extend(Allowances::empty(H160([0x02; 20])))
            .is_err());
    }

    #[test]
    fn approval_encode_interaction() {
        assert_eq!(Approval::AllowanceSufficient.encode(), vec![]);

        let token = H160([0x01; 20]);
        let spender = H160([0x02; 20]);
        assert_eq!(
            Approval::Approve { token, spender }.encode(),
            vec![(
                token,
                0.into(),
                Bytes(
                    hex::decode(
                        "095ea7b3\
                         0000000000000000000000000202020202020202020202020202020202020202\
                         ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    )
                    .unwrap()
                )
            )]
        );
    }

    fn allowance_call_data(owner: H160, spender: H160) -> web3::types::Bytes {
        let token = dummy_contract!(ERC20, H160::zero());
        token.allowance(owner, spender).m.tx.data.unwrap()
    }

    fn allowance_return_data(value: U256) -> Value {
        json!(web3::types::Bytes(abi::encode(&[Token::Uint(value)])))
    }

    #[tokio::test]
    async fn fetch_skips_failed_allowance_calls() {
        let owner = H160([1; 20]);
        let spender = H160([2; 20]);

        let web3 = mock::web3();
        web3.transport()
            .mock()
            .expect_execute_batch()
            .returning(move |calls| {
                Ok(calls
                    .into_iter()
                    .map(|(method, params)| {
                        assert_eq!(method, "eth_call");

                        let call =
                            serde_json::from_value::<CallRequest>(params[0].clone()).unwrap();
                        assert_eq!(call.data.unwrap(), allowance_call_data(owner, spender));

                        match call.to.unwrap() {
                            addr!("1111111111111111111111111111111111111111") => {
                                Ok(allowance_return_data(1337.into()))
                            }
                            addr!("2222222222222222222222222222222222222222") => {
                                Err(web3::Error::Decoder("test error".to_string()))
                            }
                            token => panic!("call to unexpected token {:?}", token),
                        }
                    })
                    .collect())
            });

        let allowances = fetch_allowances(
            web3,
            hashset![H160([0x11; 20]), H160([0x22; 20])],
            owner,
            spender,
        )
        .await
        .unwrap();

        assert_eq!(allowances, hashmap! { H160([0x11; 20]) => 1337.into() });
    }

    #[tokio::test]
    async fn fetch_fails_on_batch_errors() {
        let web3 = mock::web3();
        web3.transport()
            .mock()
            .expect_execute_batch()
            .returning(|_| Err(web3::Error::Decoder("test error".to_string())));

        let allowances = fetch_allowances(
            web3,
            hashset![H160([0x11; 20]), H160([0x22; 20])],
            H160([1; 20]),
            H160([2; 20]),
        )
        .await;

        assert!(allowances.is_err());
    }
}
