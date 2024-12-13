use {
    contracts::{Hub, web3::dummy},
    ethcontract::{
        web3::{
            self,
            types::Bytes,
            Transport, BatchTransport,
        },
        Bytes as EthBytes,
        H160, U256,
    },
    shared::interaction::{EncodedInteraction, Interaction},
    crate::interactions::pathfinder::TransferStep,
    anyhow::{anyhow, Result},
    crate::interactions::allowances::{
        AllowanceManaging,
        Allowances,
        ApprovalRequest,
        Approval,
    },
    std::collections::HashMap,
    tracing,
};

#[derive(thiserror::Error, Debug)]
pub enum CircleUbiError {
    #[error("no steps were provided")]
    EmptySteps,
    #[error("first step from ({actual}) does not match expected src ({expected})")]
    InvalidStart { actual: H160, expected: H160 },
    #[error("last step to ({actual}) does not match expected dst ({expected})")]
    InvalidEnd { actual: H160, expected: H160 },
    #[error("could not parse value {0}")]
    InvalidValue(String),
    #[error("insufficient allowance for transfer: token: {token:?}, owner: {owner:?}, spender: {spender:?}, required: {required}")]
    InsufficientAllowance {
        token: H160,
        owner: H160,
        spender: H160,
        required: U256,
    },
    #[error("trust relationship missing or inadequate for {from:?} to trust {to:?} for amount {value}")]
    TrustFailed {
        from: H160,
        to: H160,
        value: U256,
    },
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Clone, Debug)]
pub struct CircleUbiTransitiveInteraction {
    /// The token initially sent (input)
    pub input_token: H160,
    /// The token that should be received at the end (output)
    pub output_token: H160,
    /// Intermediary tokens encountered along the route. This is optional if steps are used directly.
    pub intermediary_tokens: Vec<H160>,
    /// The total amount to route.
    pub amount: U256,
    /// The starting address for the route.
    pub src: H160,
    /// The final receiving address.
    pub dest: H160,
    /// The Hub contract address.
    pub hub_address: H160,
    /// The path steps returned by the pathfinder.
    pub transfer_steps: Vec<TransferStep>,
}

impl CircleUbiTransitiveInteraction {
    /// Old constructor: might still be useful for tests without pathfinder steps.
    pub fn new(
        input_token: H160,
        output_token: H160,
        intermediary_tokens: Vec<H160>,
        amount: U256,
        src: H160,
        dest: H160,
        hub_address: H160,
    ) -> Self {
        Self {
            input_token,
            output_token,
            intermediary_tokens,
            amount,
            src,
            dest,
            hub_address,
            transfer_steps: Vec::new(),
        }
    }

    /// New constructor that takes the pathfinder steps directly and verifies allowances and trust.
    pub async fn from_transfer_steps_verified<T>(
        input_token: H160,
        output_token: H160,
        amount: U256,
        src: H160,
        dest: H160,
        hub_address: H160,
        transfer_steps: Vec<TransferStep>,
        allowance_manager: &dyn AllowanceManaging,
        spender: H160,
        web3: &web3::Web3<T>,
    ) -> Result<Self, CircleUbiError>
    where
        T: Transport + BatchTransport + Send + Sync + 'static,
        T::Out: Send,
        T::Batch: Send,
    {
        if transfer_steps.is_empty() {
            return Err(CircleUbiError::EmptySteps);
        }

        // Validate src/dest match
        let first_step = &transfer_steps[0];
        let last_step = &transfer_steps[transfer_steps.len() - 1];

        if first_step.from != src {
            return Err(CircleUbiError::InvalidStart {
                actual: first_step.from,
                expected: src,
            });
        }

        if last_step.to != dest {
            return Err(CircleUbiError::InvalidEnd {
                actual: last_step.to,
                expected: dest,
            });
        }

        let interaction = Self {
            input_token,
            output_token,
            intermediary_tokens: Vec::new(),
            amount,
            src,
            dest,
            hub_address,
            transfer_steps,
        };

        // Verify trust relationships and allowances
        interaction.verify_path(allowance_manager, spender, web3).await?;

        Ok(interaction)
    }

    /// New constructor that takes the pathfinder steps directly.
    /// This constructor:
    /// - Sets `input_token`, `output_token`, `amount`, `src`, `dest`, `hub_address`.
    /// - Stores `transfer_steps`.
    /// The `intermediary_tokens` can be derived or left empty since we directly use `transfer_steps`.
    pub fn from_transfer_steps(
        input_token: H160,
        output_token: H160,
        amount: U256,
        src: H160,
        dest: H160,
        hub_address: H160,
        transfer_steps: Vec<TransferStep>,
    ) -> Result<Self, anyhow::Error> {
        if transfer_steps.is_empty() {
            return Err(anyhow!("No transfer steps provided"));
        }

        // Optional validation: The first step should start from `src` and the last step should end at `dest`.
        let first_step = &transfer_steps[0];
        let last_step = &transfer_steps[transfer_steps.len() - 1];

        if first_step.from != src {
            return Err(anyhow!(
                "First transfer step 'from' ({:?}) does not match expected src ({:?})",
                first_step.from,
                src
            ));
        }

        if last_step.to != dest {
            return Err(anyhow!(
                "Last transfer step 'to' ({:?}) does not match expected dest ({:?})",
                last_step.to,
                dest
            ));
        }

        Ok(Self {
            input_token,
            output_token,
            intermediary_tokens: Vec::new(),
            amount,
            src,
            dest,
            hub_address,
            transfer_steps,
        })
    }

    /// Constructs the arrays needed for `transferThrough` using `transfer_steps`.
    fn build_transfer_through_params(&self) -> Result<(Vec<H160>, Vec<H160>, Vec<H160>, Vec<U256>)> {
        let mut token_owners = Vec::new();
        let mut srcs = Vec::new();
        let mut dests = Vec::new();
        let mut wads = Vec::new();

        if self.transfer_steps.is_empty() {
            // fallback to old logic if no steps given:
            return Ok(self.build_transfer_through_params_legacy());
        }

        for step in &self.transfer_steps {
            token_owners.push(step.token_owner);
            srcs.push(step.from);
            dests.push(step.to);

            let value = U256::from_dec_str(&step.value)
                .map_err(|_| anyhow!("Invalid step value: {}", step.value))?;
            wads.push(value);
        }

        // Basic validation: all arrays must match in length
        if !(token_owners.len() == srcs.len()
            && srcs.len() == dests.len()
            && dests.len() == wads.len())
        {
            return Err(anyhow!("Mismatched array lengths in transfer steps"));
        }

        Ok((token_owners, srcs, dests, wads))
    }

    /// Legacy route construction if no transfer steps are given:
    fn build_transfer_through_params_legacy(&self) -> (Vec<H160>, Vec<H160>, Vec<H160>, Vec<U256>) {
        // This logic is only a fallback for old tests.
        let mut token_owners = Vec::new();
        let mut srcs = Vec::new();
        let mut dests = Vec::new();
        let mut wads = Vec::new();

        // If using legacy logic from earlier code:
        let first_dest = self.intermediary_tokens.first().copied().unwrap_or(self.dest);
        token_owners.push(self.input_token);
        srcs.push(self.src);
        dests.push(first_dest);
        wads.push(self.amount);

        if let Some(last_intermediary) = self.intermediary_tokens.last() {
            token_owners.push(*last_intermediary);
            srcs.push(*last_intermediary);
            dests.push(self.dest);
            wads.push(self.amount);
        }

        (token_owners, srcs, dests, wads)
    }

    /// Checks if `from` trusts `to` to relay `value` amount of tokens.
    /// Now implemented by querying the Hub contract `limits` mapping.
    /// 
    /// If `limits[to][from]` < `value`, trust fails.
    async fn check_trust_relationship<T>(
        hub_address: H160,
        web3: &web3::Web3<T>,
        from: H160,
        to: H160,
        value: U256,
    ) -> Result<bool, CircleUbiError> 
    where
        T: Transport + BatchTransport + Send + Sync + 'static,
        T::Out: Send,
        T::Batch: Send,
    {
        let hub = Hub::at(web3, hub_address);

        let trust_limit = hub
            .methods()
            .limits(to, from)
            .call()
            .await
            .map_err(|err| {
                tracing::error!(?from, ?to, ?err, "Failed to query trust limits from Hub contract");
                CircleUbiError::Internal(anyhow!("could not fetch trust limit: {:?}", err))
            })?;

        // Compare trust_limit with value
        if trust_limit >= value {
            Ok(true)
        } else {
            tracing::warn!(
                ?from, ?to, ?value, ?trust_limit,
                "Trust limit insufficient for this transfer"
            );
            Ok(false)
        }
    }

    /// Checks allowances for all steps in the transfer path.
    /// Uses the existing AllowanceManager to verify allowances.
    async fn verify_allowances(
        &self,
        allowance_manager: &dyn AllowanceManaging,
        spender: H160,
    ) -> Result<(), CircleUbiError> {
        let mut approval_requests = Vec::new();
        
        for step in &self.transfer_steps {
            let value = U256::from_dec_str(&step.value)
                .map_err(|_| CircleUbiError::InvalidValue(step.value.clone()))?;
                
            approval_requests.push(ApprovalRequest {
                token: step.token_owner,
                spender,
                amount: value,
            });
        }

        let approvals = allowance_manager
            .get_approvals(&approval_requests)
            .await
            .map_err(CircleUbiError::Internal)?;

        // If any approvals are needed, it means we have insufficient allowances
        if !approvals.is_empty() {
            let failed = &approval_requests[0]; // Report first failure
            return Err(CircleUbiError::InsufficientAllowance {
                token: failed.token,
                owner: failed.spender,
                spender,
                required: failed.amount,
            });
        }

        Ok(())
    }

    /// Verifies both trust relationships and allowances for all steps.
    /// Requires a reference to a web3 instance and a hub_address to perform trust checks.
    pub async fn verify_path<T>(
        &self,
        allowance_manager: &dyn AllowanceManaging,
        spender: H160,
        web3: &web3::Web3<T>,
    ) -> Result<(), CircleUbiError>
    where
        T: Transport + BatchTransport + Send + Sync + 'static,
        T::Out: Send,
        T::Batch: Send,
    {
        // First verify all trust relationships
        for step in &self.transfer_steps {
            let value = U256::from_dec_str(&step.value)
                .map_err(|_| CircleUbiError::InvalidValue(step.value.clone()))?;

            let trust_ok = Self::check_trust_relationship(
                self.hub_address,
                web3,
                step.from,
                step.to,
                value,
            ).await?;
            if !trust_ok {
                return Err(CircleUbiError::TrustFailed {
                    from: step.from,
                    to: step.to,
                    value,
                });
            }
        }

        // Then verify all allowances
        self.verify_allowances(allowance_manager, spender).await?;

        Ok(())
    }
}

impl Interaction for CircleUbiTransitiveInteraction {
    fn encode(&self) -> EncodedInteraction {
        // Create the contract instance
        let instance = Hub::at(&dummy(), self.hub_address);

        // Build the parameter arrays
        let (token_owners, srcs, dests, wads) = self.build_transfer_through_params()
            .expect("failed to build transfer params");

        // Encode the transferThrough call
        let calldata = instance
            .methods()
            .transfer_through(token_owners, srcs, dests, wads)
            .tx
            .data
            .expect("failed to encode transferThrough call");

        // Convert web3::types::Bytes to ethcontract::Bytes
        let bytes = EthBytes(calldata.0);
        (self.hub_address, 0.into(), bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ethrpc::{mock, Web3};
    use ethcontract::transport::DynTransport;
    use mockall::predicate;
    use serde_json::json;
    use std::collections::HashMap;
    use crate::interactions::allowances::{AllowanceManaging, Allowances, ApprovalRequest, Approval};

    #[derive(Debug)]
    struct MockAllowanceManager {
        allowances: HashMap<(H160, H160), U256>,
    }

    #[async_trait::async_trait]
    impl AllowanceManaging for MockAllowanceManager {
        async fn get_allowances(&self, tokens: std::collections::HashSet<H160>, spender: H160) -> Result<Allowances> {
            let mut allowances = HashMap::new();
            for token in tokens {
                if let Some(&amount) = self.allowances.get(&(token, spender)) {
                    allowances.insert(token, amount);
                }
            }
            Ok(Allowances::new(spender, allowances))
        }

        async fn get_approvals(&self, requests: &[ApprovalRequest]) -> Result<Vec<Approval>> {
            let mut approvals = Vec::new();
            for request in requests {
                let current = self.allowances
                    .get(&(request.token, request.spender))
                    .copied()
                    .unwrap_or_default();
                
                if current < request.amount {
                    approvals.push(Approval {
                        token: request.token,
                        spender: request.spender,
                    });
                }
            }
            Ok(approvals)
        }
    }

    fn setup_mock_transport_with_limit(limit: U256) -> mock::MockTransport {
        let mut transport = mock::MockTransport::new();
        transport
            .mock()
            .expect_execute()
            .returning(move |_, _| {
                let mut response = vec![0u8; 32];
                limit.to_big_endian(&mut response);
                Ok(json!(format!("0x{}", hex::encode(response))))
            });
        transport
    }

    // ---------------------------
    // RESTORED BASIC CONSTRUCTOR TESTS
    // ---------------------------

    #[test]
    fn test_from_transfer_steps_empty() {
        let res = CircleUbiTransitiveInteraction::from_transfer_steps(
            H160::zero(),
            H160::zero(),
            U256::one(),
            H160::zero(),
            H160::zero(),
            H160::zero(),
            vec![],
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_from_transfer_steps_src_dest_validation() {
        let steps = vec![TransferStep {
            from: H160::from_low_u64_be(0x99),
            to: H160::from_low_u64_be(0x100),
            token_owner: H160::from_low_u64_be(0x20),
            value: "100".to_string(),
        }];

        let src = H160::from_low_u64_be(0x05);
        let dest = H160::from_low_u64_be(0x06);

        let res = CircleUbiTransitiveInteraction::from_transfer_steps(
            H160::zero(),
            H160::zero(),
            U256::one(),
            src,
            dest,
            H160::zero(),
            steps,
        );

        assert!(res.is_err());
        let err_msg = format!("{:?}", res.err().unwrap());
        assert!(err_msg.contains("First transfer step 'from'"));
    }

    #[test]
    fn test_from_transfer_steps_success() {
        let input_token = H160::from_low_u64_be(0x01);
        let output_token = H160::from_low_u64_be(0x02);
        let amount = U256::from(4736_u64);
        let src = H160::from_low_u64_be(0x05);
        let dest = H160::from_low_u64_be(0x06);
        let hub_address = H160::from_low_u64_be(0x07);

        let steps = vec![
            TransferStep {
                from: src,
                to: H160::from_low_u64_be(0x10),
                token_owner: H160::from_low_u64_be(0x20),
                value: "4736".to_string(),
            },
            TransferStep {
                from: H160::from_low_u64_be(0x10),
                to: dest,
                token_owner: H160::from_low_u64_be(0x21),
                value: "4736".to_string(),
            },
        ];

        let interaction = CircleUbiTransitiveInteraction::from_transfer_steps(
            input_token,
            output_token,
            amount,
            src,
            dest,
            hub_address,
            steps.clone(),
        ).expect("should create interaction from steps");

        let (token_owners, srcs, dests, wads) =
            interaction.build_transfer_through_params().expect("build params");

        assert_eq!(token_owners, vec![steps[0].token_owner, steps[1].token_owner]);
        assert_eq!(srcs, vec![steps[0].from, steps[1].from]);
        assert_eq!(dests, vec![steps[0].to, steps[1].to]);
        assert_eq!(wads, vec![U256::from(4736), U256::from(4736)]);
    }

    // ---------------------------
    // NEW VERIFIED CONSTRUCTOR TESTS (ALREADY PRESENT)
    // ---------------------------

    #[tokio::test]
    async fn test_from_transfer_steps_verified_success() {
        let src = H160::from_low_u64_be(0x1);
        let intermediate = H160::from_low_u64_be(0x2);
        let dest = H160::from_low_u64_be(0x3);
        let token_owner = H160::from_low_u64_be(0x4);
        let spender = H160::from_low_u64_be(0x5);
        let amount = U256::from(100);

        let steps = vec![
            TransferStep {
                from: src,
                to: intermediate,
                token_owner,
                value: "100".to_string(),
            },
            TransferStep {
                from: intermediate,
                to: dest,
                token_owner,
                value: "100".to_string(),
            },
        ];

        let mock_allowance_manager = MockAllowanceManager {
            allowances: vec![
                ((token_owner, spender), U256::from(1000)),
            ].into_iter().collect(),
        };

        let transport = setup_mock_transport_with_limit(U256::from(1_000_000));
        let web3 = Web3::new(DynTransport::new(transport));

        let result = CircleUbiTransitiveInteraction::from_transfer_steps_verified(
            token_owner,
            token_owner,
            amount,
            src,
            dest,
            H160::zero(),
            steps,
            &mock_allowance_manager,
            spender,
            &web3,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_from_transfer_steps_verified_insufficient_allowance() {
        let src = H160::from_low_u64_be(0x1);
        let dest = H160::from_low_u64_be(0x2);
        let token_owner = H160::from_low_u64_be(0x3);
        let spender = H160::from_low_u64_be(0x4);
        let amount = U256::from(100);

        let steps = vec![
            TransferStep {
                from: src,
                to: dest,
                token_owner,
                value: "100".to_string(),
            },
        ];

        let mock_allowance_manager = MockAllowanceManager {
            allowances: vec![
                ((token_owner, spender), U256::from(50)), // Insufficient allowance
            ].into_iter().collect(),
        };

        let transport = setup_mock_transport_with_limit(U256::from(1_000_000));
        let web3 = Web3::new(DynTransport::new(transport));

        let result = CircleUbiTransitiveInteraction::from_transfer_steps_verified(
            token_owner,
            token_owner,
            amount,
            src,
            dest,
            H160::zero(),
            steps,
            &mock_allowance_manager,
            spender,
            &web3,
        )
        .await;

        assert!(matches!(
            result,
            Err(CircleUbiError::InsufficientAllowance { .. })
        ));
    }

    #[tokio::test]
    async fn test_from_transfer_steps_verified_insufficient_trust() {
        let src = H160::from_low_u64_be(0x1);
        let dest = H160::from_low_u64_be(0x2);
        let token_owner = H160::from_low_u64_be(0x3);
        let spender = H160::from_low_u64_be(0x4);
        let amount = U256::from(100);

        let steps = vec![
            TransferStep {
                from: src,
                to: dest,
                token_owner,
                value: "100".to_string(),
            },
        ];

        let mock_allowance_manager = MockAllowanceManager {
            allowances: vec![
                ((token_owner, spender), U256::from(1000)), // Sufficient allowance
            ].into_iter().collect(),
        };

        let transport = setup_mock_transport_with_limit(U256::from(50)); // Trust limit less than required amount
        let web3 = Web3::new(DynTransport::new(transport));

        let result = CircleUbiTransitiveInteraction::from_transfer_steps_verified(
            token_owner,
            token_owner,
            amount,
            src,
            dest,
            H160::zero(),
            steps,
            &mock_allowance_manager,
            spender,
            &web3,
        )
        .await;

        assert!(matches!(
            result,
            Err(CircleUbiError::TrustFailed { .. })
        ));
    }
}
