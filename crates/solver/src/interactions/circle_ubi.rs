use {
    contracts::{Hub, web3::dummy},
    ethcontract::{Bytes, H160, U256},
    shared::interaction::{EncodedInteraction, Interaction},
    crate::interactions::pathfinder::TransferStep,
    anyhow::{anyhow, Result},
};

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
    ) -> Result<Self> {
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

        // Intermediary tokens may not be strictly needed if we have full steps directly.
        // We'll keep them empty or derive them from the steps if desired.
        Ok(Self {
            input_token,
            output_token,
            intermediary_tokens: Vec::new(), // not strictly needed if using steps
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
        let bytes = Bytes(calldata.0);
        (self.hub_address, 0.into(), bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactions::allowances::AllowanceManager;
    use ethrpc::{mock, Web3};
    use ethcontract::transport::DynTransport;

    #[tokio::test]
    async fn test_supports_circle_ubi_returns_false() {
        let transport = mock::MockTransport::new();
        let web3 = Web3::new(DynTransport::new(transport));
        let owner = H160::zero();
        let allowance_manager = AllowanceManager::new(web3, owner);

        assert!(!allowance_manager.supports_circle_ubi().await);
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
} 