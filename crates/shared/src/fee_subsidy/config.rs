use {
    super::{FeeSubsidizing, Subsidy, SubsidyParameters},
    anyhow::Result,
    ethcontract::H160,
    model::app_id::AppId,
    std::collections::{HashMap, HashSet},
};

/// The global configured fee subsidy to use for orders.
///
/// Given an estimated fee for a trade, the mimimum fee required for an order is
/// computed using the following formula:
/// ```text
/// (estimated_fee_in_eth - fee_discount) * fee_factor * (partner_additional_fee_factor || 1.0)
/// ```
pub struct FeeSubsidyConfiguration {
    /// A flat discount nominated in the native token to discount from fees.
    ///
    /// Flat fee discounts are applied **before** any multiplicative discounts.
    pub fee_discount: f64,

    /// Minimum fee amount after applying the flat subsidy. This prevents flat
    /// fee discounts putting the fee amount below 0.
    ///
    /// Flat fee discounts are applied **before** any multiplicative discounts.
    pub min_discounted_fee: f64,

    /// A factor to multiply the estimated trading fee with in order to compute
    /// subsidized minimum fee.
    ///
    /// Fee factors are applied **after** flat fee discounts.
    pub fee_factor: f64,

    /// Liquidity order providers that get completely discounted fees.
    pub liquidity_order_owners: HashSet<H160>,

    /// Additional factors per order app ID for computing the subsidized minimum
    /// fee.
    ///
    /// Fee factors are applied **after** flat fee discounts.
    pub partner_additional_fee_factors: HashMap<AppId, f64>,
}

impl Default for FeeSubsidyConfiguration {
    fn default() -> Self {
        Self {
            fee_discount: 0.,
            fee_factor: 1.,
            min_discounted_fee: 0.,
            liquidity_order_owners: Default::default(),
            partner_additional_fee_factors: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl FeeSubsidizing for FeeSubsidyConfiguration {
    async fn subsidy(&self, parameters: SubsidyParameters) -> Result<Subsidy> {
        let liquidity_factor = if self.liquidity_order_owners.contains(&parameters.from) {
            0.
        } else {
            1.
        };
        let partner_factor = self
            .partner_additional_fee_factors
            .get(&parameters.app_data)
            .copied()
            .unwrap_or(1.);

        Ok(Subsidy {
            discount: self.fee_discount,
            min_discounted: self.min_discounted_fee,
            factor: self.fee_factor * liquidity_factor * partner_factor,
        })
    }
}
