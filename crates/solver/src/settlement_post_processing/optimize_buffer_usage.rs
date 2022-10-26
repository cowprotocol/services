use super::SettlementSimulating;
use crate::settlement::Settlement;
use shared::token_list::TokenList;
use std::sync::{Arc, RwLock};

/// If a settlement only trades trusted tokens try to optimize it by trading with internal buffers.
pub async fn optimize_buffer_usage(
    settlement: Settlement,
    market_makable_token_list: Arc<RwLock<Option<TokenList>>>,
    settlement_simulator: &impl SettlementSimulating,
) -> Settlement {
    // We don't want to buy tokens that we don't trust. If no list is set, we settle with external liquidity.
    if !market_makable_token_list
        .read()
        .unwrap()
        .as_ref()
        .map(|list| is_only_selling_trusted_tokens(&settlement, list))
        .unwrap_or(false)
    {
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

fn is_only_selling_trusted_tokens(settlement: &Settlement, token_list: &TokenList) -> bool {
    !settlement
        .traded_orders()
        .any(|order| token_list.get(&order.data.sell_token).is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::{OrderTrade, Trade};
    use maplit::hashmap;
    use model::order::{Order, OrderData};
    use primitive_types::H160;
    use shared::token_list::Token;
    use std::collections::HashMap;

    #[test]
    fn test_is_only_selling_trusted_tokens() {
        let good_token = H160::from_low_u64_be(1);
        let another_good_token = H160::from_low_u64_be(2);
        let bad_token = H160::from_low_u64_be(3);

        let token_list = TokenList::new(hashmap! {
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

        let trade = |token| OrderTrade {
            trade: Trade {
                order: Order {
                    data: OrderData {
                        sell_token: token,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let settlement = Settlement::with_trades(
            HashMap::new(),
            vec![trade(good_token), trade(another_good_token)],
            vec![],
        );
        assert!(is_only_selling_trusted_tokens(&settlement, &token_list));

        let settlement = Settlement::with_trades(
            HashMap::new(),
            vec![
                trade(good_token),
                trade(another_good_token),
                trade(bad_token),
            ],
            vec![],
        );
        assert!(!is_only_selling_trusted_tokens(&settlement, &token_list));
    }
}
