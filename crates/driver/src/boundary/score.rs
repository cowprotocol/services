use {
    crate::{
        boundary,
        domain::{
            competition::{
                self,
                score::{
                    self,
                    risk::{ObjectiveValue, SuccessProbability},
                },
            },
            eth,
        },
        infra::Ethereum,
        util::conv::u256::U256Ext,
    },
    number::conversions::{big_rational_to_u256, u256_to_big_rational},
    score::Score,
    shared::external_prices::ExternalPrices,
    solver::settlement_rater::ScoreCalculator,
    std::collections::HashMap,
};

pub fn score(
    score_cap: Score,
    objective_value: ObjectiveValue,
    success_probability: SuccessProbability,
    failure_cost: eth::GasCost,
) -> Result<Score, score::Error> {
    match ScoreCalculator::new(score_cap.0.get().to_big_rational()).compute_score(
        &objective_value.0.get().to_big_rational(),
        failure_cost.get().0.to_big_rational(),
        success_probability.0,
    ) {
        Ok(score) => Ok(score.try_into()?),
        Err(err) => Err(boundary::Error::from(err).into()),
    }
}

/// Converts a solver provided score denominated in surplus tokens, to a
/// competition score denominated in native token.
pub fn to_native_score(
    score: HashMap<eth::TokenAddress, eth::TokenAmount>,
    eth: &Ethereum,
    auction: &competition::Auction,
) -> Result<competition::Score, score::Error> {
    let prices = ExternalPrices::try_from_auction_prices(
        eth.contracts().weth().address(),
        auction
            .tokens()
            .iter()
            .filter_map(|token| {
                token
                    .price
                    .map(|price| (token.address.into(), price.into()))
            })
            .collect(),
    )?;

    let native_score = score
        .iter()
        .filter_map(|(token, amount)| {
            let native_amount =
                prices.try_get_native_amount(token.0 .0, u256_to_big_rational(&amount.0))?;
            Some((token.0 .0, big_rational_to_u256(&native_amount).ok()?))
        })
        .fold(eth::U256::zero(), |acc, (_, amount)| acc + amount);

    println!("native_score: {:?}", native_score);
    Ok(Score(native_score.try_into()?))
}
