use super::SettlementSimulating;
use crate::settlement::Settlement;
use shared::token_list::AutoUpdatingTokenList;

/// If a settlement only trades trusted tokens try to optimize it by trading with internal buffers.
pub async fn optimize_buffer_usage(
    settlement: Settlement,
    market_makable_token_list: AutoUpdatingTokenList,
    settlement_simulator: &impl SettlementSimulating,
) -> Settlement {
    // We don't want to buy tokens that we don't trust. If no list is set, we settle with external liquidity.
    if !is_only_selling_trusted_tokens(&settlement, &market_makable_token_list) {
        return settlement;
    }

    let optimized_settlement = settlement.clone().without_onchain_liquidity();

    if settlement_simulator
        .settlement_would_succeed(optimized_settlement.clone())
        .await
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
    let market_makable_token_list = market_makable_token_list.addresses();
    !settlement
        .traded_orders()
        .any(|order| !market_makable_token_list.contains(&order.data.sell_token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::Trade;
    use maplit::hashmap;
    use model::order::{Order, OrderData};
    use primitive_types::H160;
    use shared::token_list::Token;

    #[test]
    fn test_is_only_selling_trusted_tokens() {
        let good_token = H160::from_low_u64_be(1);
        let another_good_token = H160::from_low_u64_be(2);
        let bad_token = H160::from_low_u64_be(3);

        let token_list = AutoUpdatingTokenList::new(hashmap! {
            good_token => Token {
                address: good_token,
                symbol: "Foo".into(),
                name: "FooCoin".into(),
                decimals: 18,
            },
            another_good_token => Token {
                address: another_good_token,
                symbol: "Bar".into(),
                name: "BarCoin".into(),
                decimals: 18,
            }
        });

        let trade = |token| Trade {
            order: Order {
                data: OrderData {
                    sell_token: token,
                    sell_amount: 1.into(),
                    buy_amount: 1.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 1.into(),
            ..Default::default()
        };

        let settlement =
            Settlement::with_default_prices(vec![trade(good_token), trade(another_good_token)]);
        assert!(is_only_selling_trusted_tokens(&settlement, &token_list));

        let settlement = Settlement::with_default_prices(vec![
            trade(good_token),
            trade(another_good_token),
            trade(bad_token),
        ]);
        assert!(!is_only_selling_trusted_tokens(&settlement, &token_list));
    }
}
