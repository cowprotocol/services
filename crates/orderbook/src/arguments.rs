use anyhow::{anyhow, Context, Result};
use model::app_id::AppId;
use primitive_types::{H160, U256};
use reqwest::Url;
use shared::{
    arguments::display_option, bad_token::token_owner_finder, fee_subsidy::cow_token::SubsidyTiers,
    http_client, price_estimation::PriceEstimatorType, rate_limiter::RateLimitingStrategy,
};
use std::{collections::HashMap, net::SocketAddr, num::NonZeroUsize, time::Duration};

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(flatten)]
    pub http_client: http_client::Arguments,

    #[clap(flatten)]
    pub token_owner_finder: token_owner_finder::Arguments,

    /// A tracing Ethereum node URL to connect to, allowing a separate node URL
    /// to be used exclusively for tracing calls.
    #[clap(long, env)]
    pub tracing_node_url: Option<Url>,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "60",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub min_order_validity_period: Duration,

    /// The maximum amount of time in seconds an order can be valid for. Defaults to 3 hours. This
    /// restriction does not apply to liquidity owner orders or presign orders.
    #[clap(
        long,
        env,
        default_value = "10800",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub max_order_validity_period: Duration,

    /// The time period an EIP1271-quote request is valid.
    #[clap(
        long,
        env,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub eip1271_onchain_quote_validity_seconds: Duration,

    /// The time period an PRESIGN-quote request is valid.
    #[clap(
        long,
        env,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub presign_onchain_quote_validity_seconds: Duration,

    /// The amount of time in seconds a classification of a token into good or bad is valid for.
    #[clap(
        long,
        env,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub token_quality_cache_expiry: Duration,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// List of token addresses that should be allowed regardless of whether the bad token detector
    /// thinks they are bad. Base tokens are automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: usize,

    /// Enable EIP-1271 orders.
    #[clap(long, env)]
    pub enable_eip1271_orders: bool,

    /// Enable pre-sign orders. Pre-sign orders are accepted into the database without a valid
    /// signature, so this flag allows this feature to be turned off if malicious users are
    /// abusing the database by inserting a bunch of order rows that won't ever be valid.
    /// This flag can be removed once DDoS protection is implemented.
    #[clap(long, env)]
    pub enable_presign_orders: bool,

    /// If solvable orders haven't been successfully updated in this many blocks attempting
    /// to get them errors and our liveness check fails.
    #[clap(long, default_value = "24")]
    pub solvable_orders_max_update_age_blocks: u64,

    /// A flat fee discount denominated in the network's native token (i.e. Ether for Mainnet).
    ///
    /// Note that flat fee discounts are applied BEFORE any multiplicative factors from either
    /// `--fee-factor` or `--partner-additional-fee-factors` configuration.
    #[clap(long, env, default_value = "0")]
    pub fee_discount: f64,

    /// The minimum value for the discounted fee in the network's native token (i.e. Ether for
    /// Mainnet).
    ///
    /// Note that this minimum is applied BEFORE any multiplicative factors from either
    /// `--fee-factor` or `--partner-additional-fee-factors` configuration.
    #[clap(long, env, default_value = "0")]
    pub min_discounted_fee: f64,

    /// Gas Fee Factor: 1.0 means cost is forwarded to users alteration, 0.9 means there is a 10%
    /// subsidy, 1.1 means users pay 10% in fees than what we estimate we pay for gas.
    #[clap(long, env, default_value = "1", parse(try_from_str = shared::arguments::parse_unbounded_factor))]
    pub fee_factor: f64,

    /// Used to specify additional fee subsidy factor based on app_ids contained in orders.
    /// Should take the form of a json string as shown in the following example:
    ///
    /// '0x0000000000000000000000000000000000000000000000000000000000000000:0.5,$PROJECT_APP_ID:0.7'
    ///
    /// Furthermore, a value of
    /// - 1 means no subsidy and is the default for all app_data not contained in this list.
    /// - 0.5 means that this project pays only 50% of the estimated fees.
    #[clap(
        long,
        env,
        default_value = "",
        parse(try_from_str = parse_partner_fee_factor),
    )]
    pub partner_additional_fee_factors: HashMap<AppId, f64>,

    /// Used to configure how much of the regular fee a user should pay based on their
    /// COW + VCOW balance in base units on the current network.
    ///
    /// The expected format is "10:0.75,150:0.5" for 2 subsidy tiers.
    /// A balance of [10,150) COW will cause you to pay 75% of the regular fee and a balance of
    /// [150, inf) COW will cause you to pay 50% of the regular fee.
    #[clap(long, env)]
    pub cow_fee_factors: Option<SubsidyTiers>,

    /// The API endpoint to call the mip v2 solver for price estimation
    #[clap(long, env)]
    pub quasimodo_solver_url: Option<Url>,

    /// The API endpoint to call the yearn solver for price estimation
    #[clap(long, env)]
    pub yearn_solver_url: Option<Url>,

    /// How long cached native prices stay valid.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub native_price_cache_max_age_secs: Duration,

    /// How many cached native token prices can be updated at most in one maintenance cycle.
    #[clap(long, env, default_value = "3")]
    pub native_price_cache_max_update_size: usize,

    /// Which estimators to use to estimate token prices in terms of the chain's native token.
    #[clap(
        long,
        env,
        default_value = "Baseline",
        arg_enum,
        use_value_delimiter = true
    )]
    pub native_price_estimators: Vec<PriceEstimatorType>,

    /// The amount in native tokens atoms to use for price estimation. Should be reasonably large so
    /// that small pools do not influence the prices. If not set a reasonable default is used based
    /// on network id.
    #[clap(
        long,
        env,
        parse(try_from_str = U256::from_dec_str)
    )]
    pub amount_to_estimate_prices_with: Option<U256>,

    #[clap(
        long,
        env,
        default_value = "Baseline",
        arg_enum,
        use_value_delimiter = true
    )]
    pub price_estimators: Vec<PriceEstimatorType>,

    /// How many successful price estimates for each order will cause a fast price estimation to
    /// return its result early.
    /// The bigger the value the more the fast price estimation performs like the optimal price
    /// estimation.
    /// It's possible to pass values greater than the total number of enabled estimators but that
    /// will not have any further effect.
    #[clap(long, env, default_value = "2")]
    pub fast_price_estimation_results_required: NonZeroUsize,

    /// Configures the back off strategy for price estimators when requests take too long.
    /// Requests issued while back off is active get dropped entirely.
    /// Needs to be passed as "<back_off_growth_factor>,<min_back_off>,<max_back_off>".
    /// back_off_growth_factor: f64 >= 1.0
    /// min_back_off: f64 in seconds
    /// max_back_off: f64 in seconds
    #[clap(long, env, verbatim_doc_comment)]
    pub price_estimation_rate_limiter: Option<RateLimitingStrategy>,

    /// The configured addresses whose orders should be considered liquidity and
    /// not regular user orders.
    ///
    /// These orders have special semantics such as not being considered in the
    /// settlements objective funtion, not receiving any surplus, and being
    /// allowed to place partially fillable orders.
    #[clap(long, env, use_value_delimiter = true)]
    pub liquidity_order_owners: Vec<H160>,

    /// The API endpoint for the Balancer SOR API for solving.
    #[clap(long, env)]
    pub balancer_sor_url: Option<Url>,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.shared)?;
        write!(f, "{}", self.http_client)?;
        write!(f, "{}", self.token_owner_finder)?;
        display_option(f, "tracing_node_url", &self.tracing_node_url)?;
        writeln!(f, "bind_address: {}", self.bind_address)?;
        writeln!(f, "db_url: SECRET")?;
        writeln!(
            f,
            "min_order_validity_period: {:?}",
            self.min_order_validity_period
        )?;
        writeln!(
            f,
            "max_order_validity_period: {:?}",
            self.max_order_validity_period
        )?;
        writeln!(
            f,
            "eip1271_onchain_quote_validity_second: {:?}",
            self.eip1271_onchain_quote_validity_seconds
        )?;
        writeln!(
            f,
            "presign_onchain_quote_validity_second: {:?}",
            self.presign_onchain_quote_validity_seconds
        )?;
        writeln!(
            f,
            "token_quality_cache_expiry: {:?}",
            self.token_quality_cache_expiry
        )?;
        writeln!(f, "unsupported_tokens: {:?}", self.unsupported_tokens)?;
        writeln!(f, "banned_users: {:?}", self.banned_users)?;
        writeln!(f, "allowed_tokens: {:?}", self.allowed_tokens)?;
        writeln!(f, "pool_cache_lru_size: {}", self.pool_cache_lru_size)?;
        writeln!(f, "enable_eip1271_orders: {}", self.enable_eip1271_orders)?;
        writeln!(f, "enable_presign_orders: {}", self.enable_presign_orders)?;
        writeln!(
            f,
            "solvable_orders_max_update_age_blocks: {}",
            self.solvable_orders_max_update_age_blocks,
        )?;
        writeln!(f, "fee_discount: {}", self.fee_discount)?;
        writeln!(f, "min_discounted_fee: {}", self.min_discounted_fee)?;
        writeln!(f, "fee_factor: {}", self.fee_factor)?;
        writeln!(
            f,
            "partner_additional_fee_factors: {:?}",
            self.partner_additional_fee_factors
        )?;
        writeln!(f, "cow_fee_factors: {:?}", self.cow_fee_factors)?;
        display_option(f, "quasimodo_solver_url", &self.quasimodo_solver_url)?;
        display_option(f, "yearn_solver_url", &self.yearn_solver_url)?;
        writeln!(
            f,
            "native_price_cache_max_age_secs: {:?}",
            self.native_price_cache_max_age_secs
        )?;
        writeln!(
            f,
            "native_price_cache_max_update_size: {}",
            self.native_price_cache_max_update_size
        )?;
        writeln!(
            f,
            "native_price_estimators: {:?}",
            self.native_price_estimators
        )?;
        display_option(
            f,
            "amount_to_estimate_prices_with",
            &self.amount_to_estimate_prices_with,
        )?;
        writeln!(f, "price_estimators: {:?}", self.price_estimators)?;
        writeln!(
            f,
            "fast_price_estimation_results_required: {}",
            self.fast_price_estimation_results_required
        )?;
        display_option(
            f,
            "price_estimation_rate_limites",
            &self.price_estimation_rate_limiter,
        )?;
        writeln!(
            f,
            "liquidity_order_owners: {:?}",
            self.liquidity_order_owners
        )?;
        display_option(f, "balancer_sor_url", &self.balancer_sor_url)?;
        Ok(())
    }
}

/// Parses a comma separated list of colon separated values representing fee factors for AppIds.
fn parse_partner_fee_factor(s: &str) -> Result<HashMap<AppId, f64>> {
    let mut res = HashMap::default();
    if s.is_empty() {
        return Ok(res);
    }
    for pair_str in s.split(',') {
        let mut split = pair_str.trim().split(':');
        let key = split
            .next()
            .ok_or_else(|| anyhow!("missing AppId"))?
            .trim()
            .parse()
            .context("failed to parse address")?;
        let value = split
            .next()
            .ok_or_else(|| anyhow!("missing value"))?
            .trim()
            .parse::<f64>()
            .context("failed to parse fee factor")?;
        if split.next().is_some() {
            return Err(anyhow!("Invalid pair lengths"));
        }
        res.insert(key, value);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;

    #[test]
    fn parse_partner_fee_factor_ok() {
        let x = "0x0000000000000000000000000000000000000000000000000000000000000000";
        let y = "0x0101010101010101010101010101010101010101010101010101010101010101";
        // without spaces
        assert_eq!(
            parse_partner_fee_factor(&format!("{}:0.5,{}:0.7", x, y)).unwrap(),
            hashmap! { AppId([0u8; 32]) => 0.5, AppId([1u8; 32]) => 0.7 }
        );
        // with spaces
        assert_eq!(
            parse_partner_fee_factor(&format!("{}: 0.5, {}: 0.7", x, y)).unwrap(),
            hashmap! { AppId([0u8; 32]) => 0.5, AppId([1u8; 32]) => 0.7 }
        );
        // whole numbers
        assert_eq!(
            parse_partner_fee_factor(&format!("{}: 1, {}: 2", x, y)).unwrap(),
            hashmap! { AppId([0u8; 32]) => 1., AppId([1u8; 32]) => 2. }
        );
    }

    #[test]
    fn parse_partner_fee_factor_err() {
        assert!(parse_partner_fee_factor("0x1:0.5,0x2:0.7").is_err());
        assert!(parse_partner_fee_factor("0x12:0.5,0x22:0.7").is_err());
        assert!(parse_partner_fee_factor(
            "0x0000000000000000000000000000000000000000000000000000000000000000:0.5:3"
        )
        .is_err());
        assert!(parse_partner_fee_factor(
            "0x0000000000000000000000000000000000000000000000000000000000000000:word"
        )
        .is_err());
    }

    #[test]
    fn parse_partner_fee_factor_ok_on_empty() {
        assert!(parse_partner_fee_factor("").unwrap().is_empty());
    }
}
