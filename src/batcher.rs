pub mod solve_pair;
pub mod submit_solution;
use crate::batcher::solve_pair::solve_pair;
use crate::batcher::submit_solution::submit_solution;
use crate::models::orderbook::OrderBook;
use crate::models::Order;
use crate::models::Solution;
use anyhow::Result;
use ethcontract::web3::types::Address;

pub async fn batch_process(orderbook: OrderBook) -> Result<()> {
    let tokens: Vec<Address> = orderbook
        .orders
        .read()
        .await
        .keys()
        .map(|t| Address::from(t.as_fixed_bytes()))
        .collect();

    let token_pairs = get_token_pairs(&tokens);
    for token_pair in token_pairs {
        let token0: Vec<Order> = orderbook
            .get_orders_for_tokens(&token_pair.0, &token_pair.1)
            .await;
        let token1: Vec<Order> = orderbook
            .get_orders_for_tokens(&token_pair.1, &token_pair.0)
            .await;
        let solution_result: Solution = solve_pair(token0, token1)?;
        submit_solution(solution_result).await?;
    }
    Ok(())
}

fn get_token_pairs(tokens: &[Address]) -> impl Iterator<Item = (Address, Address)> + '_ {
    let len = tokens.len();
    (0..(len - 1)).flat_map(move |i| ((i + 1)..len).map(move |j| (tokens[i], tokens[j])))
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    #[test]
    fn test_get_token_pairs_with_two_tokens() {
        let token_1: Address = "A193E42526F1FEA8C99AF609dcEabf30C1c29fAA".parse().unwrap();
        let token_2: Address = "E193E42526F1FEA8C99AF609dcEabf30C1c29fAA".parse().unwrap();
        let tokens = [token_1, token_2];
        let expected = vec![(token_1, token_2)];
        let result = get_token_pairs(&tokens);
        assert_eq!(expected, result.collect::<Vec<(Address, Address)>>());
    }

    #[test]
    fn test_get_token_pairs_with_three_tokens() {
        let token_1: Address = "A193E42526F1FEA8C99AF609dcEabf30C1c29fAA".parse().unwrap();
        let token_2: Address = "B193E42526F1FEA8C99AF609dcEabf30C1c29fAA".parse().unwrap();
        let token_3: Address = "C193E42526F1FEA8C99AF609dcEabf30C1c29fAA".parse().unwrap();

        let expected = vec![(token_1, token_2), (token_1, token_3), (token_2, token_3)];
        let tokens = [token_1, token_2, token_3];
        let result = get_token_pairs(&tokens);

        assert_eq!(result.collect::<Vec<(Address, Address)>>(), expected);
    }
}
