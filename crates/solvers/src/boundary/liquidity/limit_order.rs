use {
    crate::domain::liquidity::limit_order::LimitOrder,
    contracts::ethcontract::{H160, U256},
    shared::{baseline_solver::BaselineSolvable, price_estimation::gas::GAS_PER_ZEROEX_ORDER},
};

// Follows 0x's contract implementation: <https://github.com/0xProject/protocol/blob/%400x/contracts-utils%404.8.52/contracts/utils/contracts/src/v06/LibMathV06.sol#L71-L83>
impl BaselineSolvable for LimitOrder {
    fn get_amount_out(&self, out_token: H160, (in_amount, in_token): (U256, H160)) -> Option<U256> {
        if in_token != self.taker.token.0
            || out_token != self.maker.token.0
            || in_amount > self.taker.amount
        {
            return None;
        }

        in_amount
            .checked_mul(self.maker.amount)?
            .checked_div(self.taker.amount)
    }

    fn get_amount_in(&self, in_token: H160, (out_amount, out_token): (U256, H160)) -> Option<U256> {
        if out_token != self.maker.token.0
            || in_token != self.taker.token.0
            || out_amount > self.maker.amount
        {
            return None;
        }

        out_amount
            .checked_mul(self.taker.amount)?
            .checked_div(self.maker.amount)
    }

    fn gas_cost(&self) -> usize {
        GAS_PER_ZEROEX_ORDER as usize
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::{eth, liquidity::limit_order::TakerAmount},
        contracts::ethcontract::U256,
        shared::addr,
    };

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
    fn amount_out_in_round_trip() {
        let maker_amount = to_wei(300);
        let taker_amount = to_wei(100);
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
    fn amount_in_out_round_trip() {
        let maker_amount = to_wei(100);
        let taker_amount = to_wei(300);
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

    #[test]
    fn too_high_in_amount() {
        let maker_amount = to_wei(300);
        let taker_amount = to_wei(100);
        let fee_amount = to_wei(10);

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;
        let amount_in = taker_amount.checked_mul(U256::from(2)).unwrap();
        let amount_out = order.get_amount_out(out_token, (amount_in, in_token));

        assert!(amount_out.is_none());
    }

    #[test]
    fn too_high_out_amount() {
        let maker_amount = to_wei(100);
        let taker_amount = to_wei(300);
        let fee_amount = to_wei(10);

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;
        let amount_out = maker_amount.checked_mul(U256::from(2)).unwrap();
        let amount_in = order.get_amount_in(in_token, (amount_out, out_token));

        assert!(amount_in.is_none());
    }

    #[test]
    fn wrong_tokens() {
        let maker_amount = to_wei(100);
        let taker_amount = to_wei(100);
        let fee_amount = to_wei(10);

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;
        let amount = to_wei(1);
        let amount_in = order.get_amount_in(out_token, (amount, in_token));
        let amount_out = order.get_amount_out(in_token, (amount, out_token));

        assert!(amount_in.is_none());
        assert!(amount_out.is_none());
    }

    fn to_wei(base: u32) -> U256 {
        U256::from(base) * U256::exp10(18)
    }
}
