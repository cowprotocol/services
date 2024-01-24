use {
    crate::domain::liquidity::limit_order::{LimitOrder, TakerAmount},
    contracts::ethcontract::{H160, U256},
    shared::{baseline_solver::BaselineSolvable, sources::balancer_v2::swap::fixed_point::Bfp},
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
    let fee_adjusted_amount_bfp = Bfp::from_wei(fee_adjusted_amount);
    let scaled_maker_amount = Bfp::from_wei(maker_amount).mul_down(Bfp::exp10(18)).ok()?;
    let scaled_price_ratio = scaled_maker_amount
        .div_down(Bfp::from_wei(taker_amount))
        .ok()?;
    let scaled_out_amount_bfp = fee_adjusted_amount_bfp.mul_down(scaled_price_ratio).ok()?;
    scaled_out_amount_bfp
        .div_down(Bfp::exp10(18))
        .ok()
        .map(|amount| amount.as_uint256())
}

fn calculate_amount_in(
    out_amount: U256,
    maker_amount: U256,
    taker_amount: U256,
    fee: &TakerAmount,
) -> Option<U256> {
    let scaled_taker_amount = Bfp::from_wei(taker_amount).mul_down(Bfp::exp10(18)).ok()?;
    let maker_bfp = Bfp::from_wei(maker_amount);
    let scaled_price_ratio = scaled_taker_amount.div_down(maker_bfp).ok()?;
    let required_amount_before_scaling = Bfp::from_wei(out_amount)
        .mul_down(scaled_price_ratio)
        .ok()?;
    let required_amount = required_amount_before_scaling
        .div_down(Bfp::exp10(18))
        .ok()?
        .as_uint256();
    required_amount.checked_add(fee.0)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::eth, contracts::ethcontract::U256, shared::addr};

    fn create_limit_order(maker_amount: u32, taker_amount: u32, fee_amount: u32) -> LimitOrder {
        let maker = eth::Asset {
            amount: U256::from(maker_amount),
            token: eth::TokenAddress(addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        };
        let taker = eth::Asset {
            amount: U256::from(taker_amount),
            token: eth::TokenAddress(addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        };
        let fee = TakerAmount(U256::from(fee_amount));

        LimitOrder { maker, taker, fee }
    }

    #[test]
    fn test_amount_out_in_round_trip() {
        let maker_amount: u32 = 200;
        let taker_amount: u32 = 100;
        let fee_amount: u32 = 10;
        let desired_in_amount: u32 = 50;

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;

        let amount_out = order
            .get_amount_out(out_token, (U256::from(desired_in_amount), in_token))
            .unwrap();
        let amount_in = order
            .get_amount_in(in_token, (amount_out, out_token))
            .unwrap();

        assert_eq!(amount_in, U256::from(desired_in_amount));
    }

    #[test]
    fn test_amount_in_out_round_trip() {
        let maker_amount: u32 = 100;
        let taker_amount: u32 = 200;
        let fee_amount: u32 = 10;
        let desired_out_amount: u32 = 50;

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;

        let amount_in = order
            .get_amount_in(in_token, (U256::from(desired_out_amount), out_token))
            .unwrap();
        let amount_out = order
            .get_amount_out(out_token, (amount_in, in_token))
            .unwrap();

        assert_eq!(amount_out, U256::from(desired_out_amount));
    }
}
