use {
    crate::domain::{
        competition::auction::Prices,
        eth,
        liquidity::{ExactOutput, MaxInput},
    },
    ethcontract::U256,
    num::{BigRational, CheckedDiv, CheckedMul},
    number::conversions::big_rational_to_u256,
    shared::conversions::U256Ext,
};

#[derive(Clone)]
pub struct Parameters {
    /// The maximum relative slippage factor.
    pub relative: BigRational,
    /// The maximum absolute slippage in native tokens.
    pub max: Option<eth::U256>,
    /// The minimum absolute slippage in native tokens.
    pub min: Option<eth::U256>,
    pub prices: Prices,
}

#[derive(Debug)]
pub struct Interaction {
    pub input: eth::Asset,
    pub output: eth::Asset,
}

impl Parameters {
    /// Apply the slippage parameters to the given liquidity increasing the in
    /// amount by the appropriate slippage while keeping the out amount the
    /// same.
    pub fn apply_to(
        &self,
        interaction: &Interaction,
    ) -> Result<(MaxInput, ExactOutput), super::error::Math> {
        // It is possible for liquidity to use tokens that don't have prices. In order
        // to handle these cases, we do in order:
        // 1. Compute the capped slippage using the sell token amount
        // 2. If no sell token price is available, compute the capped slippage using the
        //    buy token amount
        // 3. Fall back to using the default relative slippage without capping
        let slippage = if let Some(price) = self.prices.get(&interaction.input.token) {
            let amount = price.in_eth(interaction.input.amount);
            let relative = amount.0.to_big_rational() * &self.relative;
            let relative =
                big_rational_to_u256(&relative).map_err(|_| super::error::Math::DivisionByZero)?;

            // Final slippage considers min/max caps
            let slippage = num::clamp(
                relative,
                self.min.unwrap_or_default(),
                self.max.unwrap_or(U256::max_value()),
            );

            tracing::debug!(
                input_token = ?interaction.input.token,
                ?relative,
                ?slippage,
                "using liquidity input token for capped surplus",
            );

            // Convert back to input token amount
            price.from_eth(eth::Ether(slippage))
        } else if let Some(price) = self.prices.get(&interaction.output.token) {
            let amount = price.in_eth(interaction.output.amount);
            let relative = amount.0.to_big_rational() * &self.relative;
            let relative =
                big_rational_to_u256(&relative).map_err(|_| super::error::Math::DivisionByZero)?;

            // Final slippage considers min/max caps
            let slippage = num::clamp(
                relative,
                self.min.unwrap_or_default(),
                self.max.unwrap_or(U256::max_value()),
            );

            tracing::debug!(
                input_token = ?interaction.input.token,
                ?relative,
                ?slippage,
                "using liquidity output token for capped surplus",
            );

            // Convert back to input token amount
            price
                .from_eth(eth::Ether(slippage))
                .checked_mul(&interaction.input.amount)
                .ok_or(super::error::Math::Overflow)?
                .checked_div(&interaction.output.amount)
                .ok_or(super::error::Math::DivisionByZero)?
        } else {
            tracing::warn!(
                input_token = ?interaction.input.token,
                output_token = ?interaction.output.token,
                "unable to compute capped slippage; falling back to relative slippage",
            );
            let relative = interaction.input.amount.0.to_big_rational() * &self.relative;
            big_rational_to_u256(&relative)
                .map_err(|_| super::error::Math::DivisionByZero)?
                .into()
        };

        tracing::debug!(?interaction, ?slippage, "applying slippage to liquidity",);
        Ok((
            MaxInput(eth::Asset {
                amount: interaction.input.amount + slippage,
                ..interaction.input
            }),
            ExactOutput(interaction.output),
        ))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::eth::Asset, num::rational::Ratio};

    const GNO: eth::H160 = eth::H160(hex_literal::hex!(
        "6810e776880c02933d47db1b9fc05908e5386b96"
    ));

    const USDC: eth::H160 = eth::H160(hex_literal::hex!(
        "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    ));

    #[test]
    fn test_input_price() {
        let interaction = Interaction {
            input: Asset {
                token: GNO.into(),
                // 1GNO
                amount: eth::U256::exp10(18).into(),
            },
            output: Asset {
                token: USDC.into(),
                // 200 USDC
                amount: (U256::from(200) * eth::U256::exp10(6)).into(),
            },
        };
        let prices = maplit::hashmap! {
            // 0.1 ETH
            GNO.into() => eth::U256::exp10(17).into(),
        };

        // no cap
        let slippage = Parameters {
            relative: Ratio::from_float(0.1).unwrap(),
            max: None,
            min: None,
            prices,
        };
        let (max_input, exact_output) = slippage.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(17)).into()
        );
        assert_eq!(exact_output.0.amount, interaction.output.amount);

        // min cap
        let min_cap = Parameters {
            // 0.1 ETH (1 GNO)
            min: Some(eth::U256::exp10(17)),
            ..slippage.clone()
        };
        let (max_input, _) = min_cap.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(18)).into()
        );

        // max cap
        let max_cap = Parameters {
            // 0.001 ETH (0.01 GNO)
            max: Some(eth::U256::exp10(15)),
            ..slippage
        };
        let (max_input, _) = max_cap.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(16)).into()
        );
    }

    #[test]
    fn test_output_price() {
        let interaction = Interaction {
            input: Asset {
                token: GNO.into(),
                // 1GNO
                amount: eth::U256::exp10(18).into(),
            },
            output: Asset {
                token: USDC.into(),
                // 200 USDC
                amount: (U256::from(200) * eth::U256::exp10(6)).into(),
            },
        };
        let prices = maplit::hashmap! {
            // $4000 per ETH (1 USD = 0.0005 ETH), 6 decimals
            USDC.into() => (U256::from(5) * eth::U256::exp10(26)).into(),
        };

        // no cap
        let slippage = Parameters {
            relative: Ratio::from_float(0.1).unwrap(),
            max: None,
            min: None,
            prices,
        };
        let (max_input, exact_output) = slippage.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(17)).into()
        );
        assert_eq!(exact_output.0.amount, interaction.output.amount);

        // min cap
        let min_cap = Parameters {
            // 0.1 ETH (1 GNO)
            min: Some(eth::U256::exp10(17)),
            ..slippage.clone()
        };
        let (max_input, _) = min_cap.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(18)).into()
        );

        // max cap
        let max_cap = Parameters {
            // 0.001 ETH (0.01 GNO)
            max: Some(eth::U256::exp10(15)),
            ..slippage
        };
        let (max_input, _) = max_cap.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(16)).into()
        );
    }

    #[test]
    fn test_no_price() {
        let slippage = Parameters {
            relative: Ratio::from_float(1.).unwrap(),
            max: Some(eth::U256::exp10(16)),
            min: Some(eth::U256::exp10(18)),
            prices: Default::default(),
        };
        let interaction = Interaction {
            input: Asset {
                token: GNO.into(),
                // 1GNO
                amount: eth::U256::exp10(18).into(),
            },
            output: Asset {
                token: USDC.into(),
                // 200 USDC
                amount: (U256::from(200) * eth::U256::exp10(6)).into(),
            },
        };

        // Relative slippage without cap
        let (max_input, exact_output) = slippage.apply_to(&interaction).unwrap();
        assert_eq!(
            max_input.0.amount,
            (eth::U256::exp10(18) + eth::U256::exp10(18)).into()
        );
        assert_eq!(exact_output.0.amount, interaction.output.amount);
    }
}
