use {
    crate::{
        boundary,
        domain::{competition::solution, eth, quote},
        infra::blockchain::Ethereum,
    },
    shared::external_prices::ExternalPrices,
    solver::{interactions::Erc20ApproveInteraction, liquidity::slippage::SlippageCalculator},
};

const DEFAULT_QUOTE_SLIPPAGE_BPS: u32 = 100; // 1%

pub fn encode_interactions(
    eth: &Ethereum,
    interactions: &[solution::Interaction],
) -> Result<Vec<quote::Interaction>, boundary::Error> {
    let slippage_calculator = SlippageCalculator::from_bps(DEFAULT_QUOTE_SLIPPAGE_BPS, None);
    let external_prices = ExternalPrices::new(eth.contracts().weth().address(), Default::default())
        .expect("empty external prices is valid");
    let slippage_context = slippage_calculator.context(&external_prices);

    let mut encoded_interactions = vec![];

    for interaction in interactions {
        let internalize = interaction.internalize();

        let allowances = interaction.allowances();
        for allowance in allowances {
            // When encoding approvals for quotes, reset the allowance instead
            // of just setting it. This is required as some tokens only allow
            // you to approve a non-0 value if the allowance was 0 to begin
            // with, such as Tether USD.
            //
            // Alternatively, we could check existing allowances and only encode
            // the approvals if needed, but this would only result in small gas
            // optimizations which is mostly inconsequential for quotes and not
            // worth the performance hit.
            for amount in [eth::U256::zero(), eth::U256::MAX] {
                let approval = Erc20ApproveInteraction {
                    token: eth.contract_at(allowance.0.token.into()),
                    spender: allowance.0.spender.into(),
                    amount,
                };
                let (target, value, call_data) = approval.as_encoded();
                encoded_interactions.push(quote::Interaction {
                    target: eth::Address(target),
                    value: eth::Ether(value),
                    call_data: crate::util::Bytes(call_data.0),
                    internalize,
                    inputs: vec![],
                });
            }
        }

        let inputs = interaction.inputs().into_iter().map(|i| i.token).collect();
        let boundary_interaction = boundary::settlement::to_boundary_interaction(
            &slippage_context,
            eth.contracts().settlement().address().into(),
            interaction,
        )?;

        encoded_interactions.push(quote::Interaction {
            target: eth::Address(boundary_interaction.target),
            value: eth::Ether(boundary_interaction.value),
            call_data: crate::util::Bytes(boundary_interaction.call_data),
            internalize,
            inputs,
        });
    }

    Ok(encoded_interactions)
}
