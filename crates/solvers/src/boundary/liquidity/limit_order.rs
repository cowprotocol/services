use {
    crate::domain::liquidity::limit_order::{LimitOrder, TakerAmount},
    contracts::ethcontract::{H160, U256},
    shared::baseline_solver::BaselineSolvable,
};

impl BaselineSolvable for LimitOrder {
    fn get_amount_out(&self, out_token: H160, (in_amount, in_token): (U256, H160)) -> Option<U256> {
        if in_token == self.taker.token.0 && out_token == self.maker.token.0 {
            calculate_amount_out(in_amount, self.maker.amount, self.taker.amount, &self.fee)
        } else {
            None
        }
    }

    fn get_amount_in(&self, in_token: H160, (out_amount, out_token): (U256, H160)) -> Option<U256> {
        if out_token == self.maker.token.0 && in_token == self.taker.token.0 {
            calculate_amount_in(out_amount, self.maker.amount, self.taker.amount, &self.fee)
        } else {
            None
        }
    }

    fn gas_cost(&self) -> usize {
        0
    }
}

fn calculate_amount_out(
    in_amount: U256,
    maker_amount: U256,
    taker_amount: U256,
    fee: &TakerAmount,
) -> Option<U256> {
    let fee_adjusted_amount = in_amount.checked_sub(fee.0)?;
    if maker_amount > taker_amount {
        let price_ratio = maker_amount.checked_div(taker_amount)?;
        fee_adjusted_amount.checked_mul(price_ratio)
    } else {
        let inverse_price_ratio = taker_amount.checked_div(maker_amount)?;
        fee_adjusted_amount.checked_div(inverse_price_ratio)
    }
}

fn calculate_amount_in(
    out_amount: U256,
    maker_amount: U256,
    taker_amount: U256,
    fee: &TakerAmount,
) -> Option<U256> {
    if maker_amount > taker_amount {
        let inverse_price_ratio = maker_amount.checked_div(taker_amount)?;
        let required_amount_before_fee = out_amount.checked_div(inverse_price_ratio)?;
        required_amount_before_fee.checked_add(fee.0)
    } else {
        let price_ratio = taker_amount.checked_div(maker_amount)?;
        let intermediate_amount = out_amount.checked_mul(price_ratio)?;
        intermediate_amount.checked_add(fee.0)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::eth, contracts::ethcontract::U256, shared::addr};

    fn create_limit_order(maker_amount: U256, taker_amount: U256, fee_amount: U256) -> LimitOrder {
        let maker = eth::Asset {
            amount: maker_amount,
            token: eth::TokenAddress(addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        };
        let taker = eth::Asset {
            amount: taker_amount,
            token: eth::TokenAddress(addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        };
        let fee = TakerAmount(fee_amount);

        LimitOrder { maker, taker, fee }
    }

    #[test]
    fn test_amount_out_in_round_trip() {
        let maker_amount = to_wei(321);
        let taker_amount = to_wei(123);
        let fee_amount = to_wei(10);
        let desired_in_amount = to_wei(50);

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;

        let amount_out = order
            .get_amount_out(out_token, (desired_in_amount, in_token))
            .unwrap();
        let amount_in = order
            .get_amount_in(in_token, (amount_out, out_token))
            .unwrap();

        assert_eq!(amount_in, desired_in_amount);
    }

    #[test]
    fn test_amount_in_out_round_trip() {
        let maker_amount = to_wei(123);
        let taker_amount = to_wei(321);
        let fee_amount = to_wei(10);
        let desired_out_amount = to_wei(50);

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;

        let amount_in = order
            .get_amount_in(in_token, (desired_out_amount, out_token))
            .unwrap();
        let amount_out = order
            .get_amount_out(out_token, (amount_in, in_token))
            .unwrap();

        assert_eq!(amount_out, desired_out_amount);
    }

    fn to_wei_with_exp(base: u32, exp: usize) -> U256 {
        U256::from(base) * U256::exp10(exp)
    }

    fn to_wei(base: u32) -> U256 {
        to_wei_with_exp(base, 18)
    }
}
