use {
    contracts::{Hub, web3::dummy},
    ethcontract::{Bytes, H160, U256},
    shared::interaction::{EncodedInteraction, Interaction},
};

#[derive(Clone, Debug)]
pub struct CircleUbiTransitiveInteraction {
    pub input_token: H160,
    pub output_token: H160,
    pub intermediary_tokens: Vec<H160>,
    pub amount: U256,
    pub src: H160,
    pub dest: H160,
    pub hub_address: H160,
}

impl CircleUbiTransitiveInteraction {
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
        }
    }

    /// Constructs the arrays needed for transferThrough
    fn build_transfer_through_params(&self) -> (Vec<H160>, Vec<H160>, Vec<H160>, Vec<U256>) {
        // For a path: src -> intermediary_1 -> ... -> intermediary_n -> dest
        // We need arrays of equal length for tokenOwners, srcs, dests, and wads
        let mut token_owners = Vec::new();
        let mut srcs = Vec::new();
        let mut dests = Vec::new();
        let mut wads = Vec::new();

        // First hop: src -> first_intermediary (or dest if no intermediaries)
        let first_dest = self.intermediary_tokens.first().copied().unwrap_or(self.dest);
        token_owners.push(self.input_token);
        srcs.push(self.src);
        dests.push(first_dest);
        wads.push(self.amount);

        // Intermediate hops through the path
        for i in 0..self.intermediary_tokens.len().saturating_sub(1) {
            token_owners.push(self.intermediary_tokens[i]);
            srcs.push(self.intermediary_tokens[i]);
            dests.push(self.intermediary_tokens[i + 1]);
            wads.push(self.amount); // Note: In reality, amounts might need conversion rates
        }

        // Final hop: last_intermediary -> dest (if there were intermediaries)
        if let Some(last_intermediary) = self.intermediary_tokens.last() {
            token_owners.push(*last_intermediary);
            srcs.push(*last_intermediary);
            dests.push(self.dest);
            wads.push(self.amount); // Note: In reality, amounts might need conversion rates
        }

        (token_owners, srcs, dests, wads)
    }
}

impl Interaction for CircleUbiTransitiveInteraction {
    fn encode(&self) -> EncodedInteraction {
        // Create the contract instance
        let instance = Hub::at(&dummy(), self.hub_address);

        // Build the parameter arrays
        let (token_owners, srcs, dests, wads) = self.build_transfer_through_params();

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
    fn test_circle_ubi_interaction_creation() {
        let input_token = H160::from_low_u64_be(0x01);
        let output_token = H160::from_low_u64_be(0x02);
        let intermediary_tokens = vec![
            H160::from_low_u64_be(0x03),
            H160::from_low_u64_be(0x04),
        ];
        let amount = U256::from(1000_u64);
        let src = H160::from_low_u64_be(0x05);
        let dest = H160::from_low_u64_be(0x06);
        let hub_address = H160::from_low_u64_be(0x07);

        let interaction = CircleUbiTransitiveInteraction::new(
            input_token,
            output_token,
            intermediary_tokens.clone(),
            amount,
            src,
            dest,
            hub_address,
        );

        assert_eq!(interaction.input_token, input_token);
        assert_eq!(interaction.output_token, output_token);
        assert_eq!(interaction.intermediary_tokens, intermediary_tokens);
        assert_eq!(interaction.amount, amount);
        assert_eq!(interaction.src, src);
        assert_eq!(interaction.dest, dest);
        assert_eq!(interaction.hub_address, hub_address);
    }

    #[test]
    fn test_build_transfer_through_params() {
        let input_token = H160::from_low_u64_be(0x01);
        let output_token = H160::from_low_u64_be(0x02);
        let intermediary_tokens = vec![H160::from_low_u64_be(0x03)];
        let amount = U256::from(1000_u64);
        let src = H160::from_low_u64_be(0x05);
        let dest = H160::from_low_u64_be(0x06);
        let hub_address = H160::from_low_u64_be(0x07);

        let interaction = CircleUbiTransitiveInteraction::new(
            input_token,
            output_token,
            intermediary_tokens.clone(),
            amount,
            src,
            dest,
            hub_address,
        );

        let (token_owners, srcs, dests, wads) = interaction.build_transfer_through_params();

        // For a single intermediary, we expect two transfers:
        // 1. src -> intermediary
        // 2. intermediary -> dest
        assert_eq!(token_owners.len(), 2);
        assert_eq!(srcs.len(), 2);
        assert_eq!(dests.len(), 2);
        assert_eq!(wads.len(), 2);

        // First transfer
        assert_eq!(token_owners[0], input_token);
        assert_eq!(srcs[0], src);
        assert_eq!(dests[0], intermediary_tokens[0]);
        assert_eq!(wads[0], amount);

        // Second transfer
        assert_eq!(token_owners[1], intermediary_tokens[0]);
        assert_eq!(srcs[1], intermediary_tokens[0]);
        assert_eq!(dests[1], dest);
        assert_eq!(wads[1], amount);
    }

    #[test]
    fn test_encode_generates_calldata() {
        let input_token = H160::from_low_u64_be(0x01);
        let output_token = H160::from_low_u64_be(0x02);
        let intermediary_tokens = vec![H160::from_low_u64_be(0x03)];
        let amount = U256::from(1000_u64);
        let src = H160::from_low_u64_be(0x05);
        let dest = H160::from_low_u64_be(0x06);
        let hub_address = H160::from_low_u64_be(0x07);

        let interaction = CircleUbiTransitiveInteraction::new(
            input_token,
            output_token,
            intermediary_tokens,
            amount,
            src,
            dest,
            hub_address,
        );

        let (target, value, calldata) = interaction.encode();

        assert_eq!(target, hub_address);
        assert_eq!(value, U256::zero());
        assert!(!calldata.0.is_empty(), "Calldata should not be empty");
    }
} 