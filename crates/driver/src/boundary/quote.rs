use {
    crate::{
        boundary,
        domain::{competition::solution, eth},
        infra::blockchain::Ethereum,
    },
    shared::http_solver::model::InternalizationStrategy,
    solver::{
        interactions::Erc20ApproveInteraction,
        liquidity::slippage::SlippageCalculator,
        settlement::external_prices::ExternalPrices,
    },
};

const DEFAULT_QUOTE_SLIPPAGE_BPS: u32 = 100; // 1%

pub fn encode_interactions(
    eth: &Ethereum,
    interactions: &[solution::Interaction],
) -> Result<Vec<eth::Interaction>, boundary::Error> {
    let slippage_calculator = SlippageCalculator::from_bps(DEFAULT_QUOTE_SLIPPAGE_BPS, None);
    let external_prices = ExternalPrices::new(eth.contracts().weth().address(), Default::default())
        .expect("empty external prices is valid");
    let slippage_context = slippage_calculator.context(&external_prices);

    let mut settlement = solver::settlement::Settlement::new(Default::default());

    for interaction in interactions {
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
            settlement
                .encoder
                .append_to_execution_plan(Erc20ApproveInteraction {
                    token: eth.contract_at(allowance.0.spender.token.into()),
                    spender: allowance.0.spender.address.into(),
                    amount: eth::U256::zero(),
                });
            settlement
                .encoder
                .append_to_execution_plan(Erc20ApproveInteraction {
                    token: eth.contract_at(allowance.0.spender.token.into()),
                    spender: allowance.0.spender.address.into(),
                    amount: eth::U256::max_value(),
                });
        }

        let boundary_interaction = boundary::settlement::to_boundary_interaction(
            &slippage_context,
            eth.contracts().settlement().address().into(),
            interaction,
        )?;
        settlement
            .encoder
            .append_to_execution_plan_internalizable(boundary_interaction, false);
    }

    let encoded_settlement = settlement.encode(InternalizationStrategy::EncodeAllInteractions);
    Ok(encoded_settlement
        .interactions
        .into_iter()
        .flatten()
        .map(|(target, value, call_data)| eth::Interaction {
            target: target.into(),
            value: value.into(),
            call_data: call_data.0,
        })
        .collect())
}
