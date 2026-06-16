use {
    alloy_primitives::{Address, U256},
    alloy_signer::SignerSync,
    alloy_signer_local::PrivateKeySigner,
    alloy_sol_types::{Eip712Domain, SolStruct, sol},
    byos::domain::{
        eip712::{self, ProposalData},
        proposal::Interaction,
    },
    solvers::domain::solution,
};

sol! {
    /// IUniswapV2Router02 swap interface.
    #[allow(missing_docs)]
    interface IUniswapV2Router {
        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] calldata path,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);

        function swapTokensForExactTokens(
            uint256 amountOut,
            uint256 amountInMax,
            address[] calldata path,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);
    }
}

pub struct ProposalBuilder {
    signer: PrivateKeySigner,
    domain: Eip712Domain,
    router: Address,
    settlement: Address,
}

impl ProposalBuilder {
    pub fn new(
        signer: PrivateKeySigner,
        domain: Eip712Domain,
        router: Address,
        settlement: Address,
    ) -> Self {
        Self {
            signer,
            domain,
            router,
            settlement,
        }
    }

    pub fn build_and_sign(
        &self,
        order_uid: &[u8; 56],
        sol: &solution::Solution,
    ) -> anyhow::Result<SignedProposal> {
        // Extract sell/buy tokens and amounts from clearing prices.
        // The solution has exactly one trade (single order solutions).
        let trade = sol
            .trades
            .first()
            .ok_or_else(|| anyhow::anyhow!("solution has no trades"))?;
        let (sell_token, buy_token) = match trade {
            solution::Trade::Fulfillment(f) => (f.order().sell.token, f.order().buy.token),
            _ => anyhow::bail!("unexpected JIT trade"),
        };

        let sell_price = sol.prices.0.get(&sell_token).copied().unwrap_or(U256::ZERO);
        let buy_price = sol.prices.0.get(&buy_token).copied().unwrap_or(U256::ZERO);

        // Encode liquidity interactions as Uniswap V2 router calls.
        let interactions = self.encode_interactions(&sol.interactions)?;

        let valid_until = (chrono::Utc::now() + chrono::Duration::minutes(5)).timestamp() as u64;
        let nonce = U256::from(chrono::Utc::now().timestamp_millis() as u64);

        let proposal_data = ProposalData {
            orderUidHash: eip712::order_uid_hash(order_uid),
            sellAmount: buy_price, // clearing prices are cross-multiplied
            buyAmount: sell_price,
            validUntil: U256::from(valid_until),
            nonce,
        };

        let signing_hash = proposal_data.eip712_signing_hash(&self.domain);
        let sig = self.signer.sign_hash_sync(&signing_hash)?;
        let signature: [u8; 65] = sig.as_bytes();

        Ok(SignedProposal {
            order_uid: *order_uid,
            sell_amount: buy_price,
            buy_amount: sell_price,
            interactions,
            valid_until,
            nonce,
            signature,
        })
    }

    fn encode_interactions(
        &self,
        interactions: &[solution::Interaction],
    ) -> anyhow::Result<Vec<Interaction>> {
        let mut encoded = Vec::new();
        let deadline = U256::from(u64::MAX); // far future

        for interaction in interactions {
            match interaction {
                solution::Interaction::Liquidity(liq) => {
                    let input = liq.input;
                    let output = liq.output;

                    // Encode router call based on whether it's sell-side or buy-side
                    let calldata = IUniswapV2Router::swapExactTokensForTokensCall {
                        amountIn: input.amount,
                        amountOutMin: output.amount,
                        path: vec![input.token.0, output.token.0],
                        to: self.settlement,
                        deadline,
                    };

                    encoded.push(Interaction {
                        target: self.router,
                        value: U256::ZERO,
                        calldata: alloy_sol_types::SolCall::abi_encode(&calldata),
                    });
                }
                solution::Interaction::Custom(custom) => {
                    encoded.push(Interaction {
                        target: custom.target,
                        value: custom.value.0,
                        calldata: custom.calldata.clone(),
                    });
                }
            }
        }

        Ok(encoded)
    }
}

pub struct SignedProposal {
    pub order_uid: [u8; 56],
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub interactions: Vec<Interaction>,
    pub valid_until: u64,
    pub nonce: U256,
    pub signature: [u8; 65],
}
