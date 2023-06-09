use {
    super::SettlementSimulating,
    crate::settlement::Settlement,
    primitive_types::H160,
    shared::token_list::AutoUpdatingTokenList,
};

/// If a settlement only trades trusted tokens try to optimize it by trading
/// with internal buffers.
pub async fn optimize_buffer_usage(
    settlement: Settlement,
    market_makable_token_list: AutoUpdatingTokenList,
    settlement_simulator: &impl SettlementSimulating,
) -> Settlement {
    // We don't want to buy tokens that we don't trust. If no list is set, we settle
    // with external liquidity.
    if !is_only_selling_trusted_tokens(&settlement, &market_makable_token_list) {
        return settlement;
    }

    // Sometimes solvers propose stable to stable trades that produce good prices
    // but require enormous gas overhead. Normally these would be discarded but
    // due to naive buffer usage rules it's technically allowed to internalize
    // these trades which gets rid of their high gas cost. That's why we disable
    // internalization of any settlement that contains a stable to stable
    // trade until we have better rules for buffer usage. This code only affects
    // Gnosis solvers.
    if some_stable_to_stable_trade(&settlement) {
        return settlement;
    }

    let optimized_settlement = settlement.clone().without_onchain_liquidity();

    if settlement_simulator
        .estimate_gas(optimized_settlement.clone())
        .await
        .is_ok()
    {
        tracing::debug!("settlement without onchain liquidity");
        return optimized_settlement;
    }

    settlement
}

fn is_only_selling_trusted_tokens(
    settlement: &Settlement,
    market_makable_token_list: &AutoUpdatingTokenList,
) -> bool {
    let market_makable_token_list = market_makable_token_list.all();
    !settlement
        .traded_orders()
        .any(|order| !market_makable_token_list.contains(&order.data.sell_token))
}

fn some_stable_to_stable_trade(settlement: &Settlement) -> bool {
    let stable_coins = [
        H160(hex_literal::hex!(
            "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48" // USDC
        )),
        H160(hex_literal::hex!(
            "6B175474E89094C44Da98b954EedeAC495271d0F" // DAI
        )),
        H160(hex_literal::hex!(
            "dAC17F958D2ee523a2206206994597C13D831ec7" // USDT
        )),
    ];
    settlement.traded_orders().any(|o| {
        stable_coins.contains(&o.data.sell_token) && stable_coins.contains(&o.data.buy_token)
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::settlement::Trade,
        model::order::{Order, OrderData},
        primitive_types::H160,
    };

    fn trade(sell_token: H160, buy_token: H160) -> Trade {
        Trade {
            order: Order {
                data: OrderData {
                    sell_token,
                    buy_token,
                    sell_amount: 1.into(),
                    buy_amount: 1.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 1.into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_is_only_selling_trusted_tokens() {
        let good_token = H160::from_low_u64_be(1);
        let another_good_token = H160::from_low_u64_be(2);
        let bad_token = H160::from_low_u64_be(3);

        let token_list =
            AutoUpdatingTokenList::new([good_token, another_good_token].into_iter().collect());

        let settlement = Settlement::with_default_prices(vec![
            trade(good_token, bad_token),
            trade(another_good_token, bad_token),
        ]);
        assert!(is_only_selling_trusted_tokens(&settlement, &token_list));

        let settlement = Settlement::with_default_prices(vec![
            trade(good_token, bad_token),
            trade(another_good_token, bad_token),
            trade(bad_token, good_token),
        ]);
        assert!(!is_only_selling_trusted_tokens(&settlement, &token_list));
    }

    #[test]
    fn prevent_stable_to_stable_trade_internalization() {
        let usdc = H160(hex_literal::hex!(
            "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        ));
        let dai = H160(hex_literal::hex!(
            "6B175474E89094C44Da98b954EedeAC495271d0F"
        ));
        let usdt = H160(hex_literal::hex!(
            "dAC17F958D2ee523a2206206994597C13D831ec7"
        ));
        let non_stable_a = H160([1; 20]);
        let non_stable_b = H160([2; 20]);

        let settlement = Settlement::with_default_prices(vec![trade(usdc, dai)]);
        assert!(some_stable_to_stable_trade(&settlement));
        let settlement = Settlement::with_default_prices(vec![trade(usdc, usdt)]);
        assert!(some_stable_to_stable_trade(&settlement));
        let settlement = Settlement::with_default_prices(vec![
            trade(usdc, usdt),
            trade(non_stable_a, non_stable_b),
        ]);
        assert!(some_stable_to_stable_trade(&settlement));
        let settlement = Settlement::with_default_prices(vec![trade(usdc, non_stable_a)]);
        assert!(!some_stable_to_stable_trade(&settlement));
        let settlement = Settlement::with_default_prices(vec![trade(non_stable_a, usdc)]);
        assert!(!some_stable_to_stable_trade(&settlement));
        let settlement = Settlement::with_default_prices(vec![trade(non_stable_a, non_stable_b)]);
        assert!(!some_stable_to_stable_trade(&settlement));
    }
}
