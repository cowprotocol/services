use {
    crate::domain::liquidity::limit_order::LimitOrder,
    alloy::primitives::{Address, U256},
    shared::{baseline_solver::BaselineSolvable, price_estimation::gas::GAS_PER_ZEROEX_ORDER},
};

// Follows 0x's contract implementation: <https://github.com/0xProject/protocol/blob/%400x/contracts-utils%404.8.52/contracts/utils/contracts/src/v06/LibMathV06.sol#L71-L83>
impl BaselineSolvable for LimitOrder {
    async fn get_amount_out(
        &self,
        out_token: Address,
        (in_amount, in_token): (U256, Address),
    ) -> Option<U256> {
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

    async fn get_amount_in(
        &self,
        in_token: Address,
        (out_amount, out_token): (U256, Address),
    ) -> Option<U256> {
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

    async fn gas_cost(&self) -> usize {
        GAS_PER_ZEROEX_ORDER as usize
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::{eth, liquidity::limit_order::TakerAmount},
        alloy::primitives::address,
        number::units::EthUnit,
    };

    fn create_limit_order(maker_amount: U256, taker_amount: U256, fee_amount: U256) -> LimitOrder {
        let maker = eth::Asset {
            amount: maker_amount,
            token: eth::TokenAddress(address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
        };
        let taker = eth::Asset {
            amount: taker_amount,
            token: eth::TokenAddress(address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        };
        let fee = TakerAmount(fee_amount);

        LimitOrder { maker, taker, fee }
    }

    #[tokio::test]
    async fn amount_out_in_round_trip() {
        let maker_amount = 300u64.eth();
        let taker_amount = 100u64.eth();
        let fee_amount = 10u64.eth();
        let desired_in_amount = 50u64.eth();

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;

        let amount_out = order
            .get_amount_out(out_token, (desired_in_amount, in_token))
            .await
            .unwrap();
        let amount_in = order
            .get_amount_in(in_token, (amount_out, out_token))
            .await
            .unwrap();

        assert_eq!(amount_in, desired_in_amount);
    }

    #[tokio::test]
    async fn amount_in_out_round_trip() {
        let maker_amount = 100u64.eth();
        let taker_amount = 300u64.eth();
        let fee_amount = 10u64.eth();
        let desired_out_amount = 50u64.eth();

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;

        let amount_in = order
            .get_amount_in(in_token, (desired_out_amount, out_token))
            .await
            .unwrap();
        let amount_out = order
            .get_amount_out(out_token, (amount_in, in_token))
            .await
            .unwrap();

        assert_eq!(amount_out, desired_out_amount);
    }

    #[tokio::test]
    async fn too_high_in_amount() {
        let maker_amount = 300u64.eth();
        let taker_amount = 100u64.eth();
        let fee_amount = 10u64.eth();

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;
        let amount_in = taker_amount.checked_mul(U256::from(2)).unwrap();
        let amount_out = order.get_amount_out(out_token, (amount_in, in_token)).await;

        assert!(amount_out.is_none());
    }

    #[tokio::test]
    async fn too_high_out_amount() {
        let maker_amount = 100u64.eth();
        let taker_amount = 300u64.eth();
        let fee_amount = 10u64.eth();

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;
        let amount_out = maker_amount.checked_mul(U256::from(2)).unwrap();
        let amount_in = order.get_amount_in(in_token, (amount_out, out_token)).await;

        assert!(amount_in.is_none());
    }

    #[tokio::test]
    async fn wrong_tokens() {
        let maker_amount = 100u64.eth();
        let taker_amount = 100u64.eth();
        let fee_amount = 10u64.eth();

        let order = create_limit_order(maker_amount, taker_amount, fee_amount);
        let out_token = order.maker.token.0;
        let in_token = order.taker.token.0;
        let amount = 1u64.eth();
        let amount_in = order.get_amount_in(out_token, (amount, in_token)).await;
        let amount_out = order.get_amount_out(in_token, (amount, out_token)).await;

        assert!(amount_in.is_none());
        assert!(amount_out.is_none());
    }
}
