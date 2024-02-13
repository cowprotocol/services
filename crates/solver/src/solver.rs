use {
    crate::{
        liquidity::Liquidity,
        settlement::Settlement,
    },
    anyhow::{anyhow, Context, Result},
    ethcontract::{errors::ExecutionError, transaction::kms, Account, PrivateKey, H160, U256},
    model::{auction::AuctionId, order::Order},
    reqwest::Url,
    shared::{
        account_balances,
        external_prices::ExternalPrices,
        http_solver::model::SimulatedTransaction,
    },
    std::{
        collections::HashMap,
        fmt::{self, Debug, Formatter},
        str::FromStr,
        time::{Duration, Instant},
    },
};

mod baseline_solver;
pub mod naive_solver;

/// A batch auction for a solver to produce a settlement for.
#[derive(Clone, Debug)]
pub struct Auction {
    /// Note that multiple consecutive driver runs may use the same ID if the
    /// previous run was unable to find a settlement.
    pub id: AuctionId,

    /// An ID that identifies a driver run.
    ///
    /// Note that this ID is not unique across multiple instances of drivers,
    /// in particular it cannot be used to uniquely identify batches across
    /// service restarts.
    pub run: u64,

    /// The GPv2 orders to match.
    pub orders: Vec<Order>,

    /// The baseline on-chain liquidity that can be used by the solvers for
    /// settling orders.
    pub liquidity: Vec<Liquidity>,

    /// On which block the liquidity got fetched.
    pub liquidity_fetch_block: u64,

    /// The current gas price estimate.
    pub gas_price: f64,

    /// The deadline for computing a solution.
    ///
    /// This can be used internally for the solver to decide when to stop
    /// trying to optimize the settlement. The caller is expected poll the solve
    /// future at most until the deadline is reach, at which point the future
    /// will be dropped.
    pub deadline: Instant,

    /// The set of external prices for this auction.
    ///
    /// The objective value is calculated with these prices so they can be
    /// relevant for solvers.
    ///
    /// External prices are garanteed to exist for all orders included in the
    /// current auction.
    pub external_prices: ExternalPrices,

    /// Balances for `orders`. Not guaranteed to have an entry for all orders
    /// because balance fetching can fail.
    pub balances: HashMap<account_balances::Query, U256>,
}

impl Default for Auction {
    fn default() -> Self {
        const SECONDS_IN_A_YEAR: u64 = 31_622_400;

        // Not actually never, but good enough...
        let never = Instant::now() + Duration::from_secs(SECONDS_IN_A_YEAR);
        Self {
            id: Default::default(),
            run: Default::default(),
            orders: Default::default(),
            liquidity: Default::default(),
            liquidity_fetch_block: Default::default(),
            gas_price: Default::default(),
            deadline: never,
            external_prices: Default::default(),
            balances: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SolverInfo {
    /// Identifier used for metrics and logging.
    pub name: String,
    /// Address used for simulating settlements of that solver.
    pub account: Account,
}

#[derive(Debug)]
pub struct Simulation {
    pub settlement: Settlement,
    pub solver: SolverInfo,
    pub transaction: SimulatedTransaction,
}

#[derive(Debug)]
pub struct SimulationWithError {
    pub simulation: Simulation,
    pub error: SimulationError,
}

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("web3 error: {0:?}")]
    Web3(#[from] ExecutionError),
    #[error("insufficient balance: needs {needs} has {has}")]
    InsufficientBalance { needs: U256, has: U256 },
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum SolverType {
    None,
    Naive,
    Baseline,
    OneInch,
    Paraswap,
    ZeroEx,
    Quasimodo,
    BalancerSor,
}

#[derive(Clone)]
pub enum SolverAccountArg {
    PrivateKey(PrivateKey),
    Kms(Arn),
    Address(H160),
}

impl Debug for SolverAccountArg {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            SolverAccountArg::PrivateKey(k) => write!(f, "PrivateKey({:?})", k.public_address()),
            SolverAccountArg::Kms(key_id) => write!(f, "KMS({key_id:?})"),
            SolverAccountArg::Address(a) => write!(f, "Address({a:?})"),
        }
    }
}

impl SolverAccountArg {
    pub async fn into_account(self, chain_id: u64) -> Account {
        match self {
            SolverAccountArg::PrivateKey(key) => Account::Offline(key, Some(chain_id)),
            SolverAccountArg::Kms(key_id) => {
                let config = ethcontract::aws_config::load_from_env().await;
                let account = kms::Account::new((&config).into(), &key_id.0)
                    .await
                    .unwrap_or_else(|_| panic!("Unable to load KMS account {key_id:?}"));
                Account::Kms(account, Some(chain_id))
            }
            SolverAccountArg::Address(address) => Account::Local(address, None),
        }
    }
}

impl FromStr for SolverAccountArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<PrivateKey>()
            .map(SolverAccountArg::PrivateKey)
            .map_err(|pk_err| anyhow!("could not parse as private key: {}", pk_err))
            .or_else(|error_chain| {
                Ok(SolverAccountArg::Address(s.parse().map_err(
                    |addr_err| {
                        error_chain.context(anyhow!("could not parse as address: {}", addr_err))
                    },
                )?))
            })
            .or_else(|error_chain: Self::Err| {
                let key_id = Arn::from_str(s).map_err(|arn_err| {
                    error_chain.context(anyhow!("could not parse as AWS ARN: {}", arn_err))
                })?;
                Ok(SolverAccountArg::Kms(key_id))
            })
            .map_err(|err: Self::Err| {
                err.context(
                    "invalid solver account, it is neither a private key, an Ethereum address, \
                     nor a KMS key",
                )
            })
    }
}

// Wrapper type for AWS ARN identifiers
#[derive(Debug, Clone)]
pub struct Arn(pub String);

impl FromStr for Arn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        // Could be more strict here, but this should suffice to catch unintended
        // configuration mistakes
        if s.starts_with("arn:aws:kms:") {
            Ok(Self(s.to_string()))
        } else {
            Err(anyhow!("Invalid ARN identifier: {}", s))
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExternalSolverArg {
    pub name: String,
    pub url: Url,
    pub account: SolverAccountArg,
    pub use_liquidity: bool,
    pub user_balance_support: UserBalanceSupport,
}

/// Whether the solver supports assigning user sell token balance to orders or
/// whether the driver needs to do it instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserBalanceSupport {
    None,
    PartiallyFillable,
    // Will be added later.
    // All,
}

impl FromStr for UserBalanceSupport {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "partially_fillable" => Ok(Self::PartiallyFillable),
            _ => Err(anyhow::anyhow!("unknown variant {}", s)),
        }
    }
}

impl FromStr for ExternalSolverArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('|');
        let name = parts.next().context("missing name")?;
        let url = parts.next().context("missing url")?;
        let account = parts.next().context("missing account")?;
        let use_liquidity = parts.next().context("missing use_liquidity")?;
        // With a default temporarily until we configure the argument in our cluster.
        let user_balance_support = parts.next().unwrap_or("none");
        Ok(Self {
            name: name.to_string(),
            url: url.parse().context("parse url")?,
            account: account.parse().context("parse account")?,
            use_liquidity: use_liquidity.parse().context("parse use_liquidity")?,
            user_balance_support: user_balance_support
                .parse()
                .context("parse user_balance_support")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq for SolverAccountArg {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (SolverAccountArg::PrivateKey(a), SolverAccountArg::PrivateKey(b)) => {
                    a.public_address() == b.public_address()
                }
                (SolverAccountArg::Address(a), SolverAccountArg::Address(b)) => a == b,
                _ => false,
            }
        }
    }

    #[test]
    fn parses_solver_account_arg() {
        assert_eq!(
            "0x4242424242424242424242424242424242424242424242424242424242424242"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::PrivateKey(PrivateKey::from_raw([0x42; 32]).unwrap())
        );
        assert_eq!(
            "0x4242424242424242424242424242424242424242"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::Address(H160([0x42; 20])),
        );

        assert!(matches!(
            "arn:aws:kms:eu-central-1:42:key/00000000-0000-0000-0000-00000000"
                .parse::<SolverAccountArg>()
                .unwrap(),
            SolverAccountArg::Kms(_)
        ));
    }

    #[test]
    fn errors_on_invalid_solver_account_arg() {
        assert!("0x010203040506070809101112131415161718192021"
            .parse::<SolverAccountArg>()
            .is_err());
        assert!("not an account".parse::<SolverAccountArg>().is_err());
    }

    #[test]
    fn parse_external_solver_arg() {
        let arg = "name|http://solver.com/|0x4242424242424242424242424242424242424242424242424242424242424242|true|partially_fillable";
        let parsed = ExternalSolverArg::from_str(arg).unwrap();
        assert_eq!(parsed.name, "name");
        assert_eq!(parsed.url.to_string(), "http://solver.com/");
        assert_eq!(
            parsed.account,
            SolverAccountArg::PrivateKey(PrivateKey::from_raw([0x42; 32]).unwrap())
        );
        assert!(parsed.use_liquidity);
        assert_eq!(
            parsed.user_balance_support,
            UserBalanceSupport::PartiallyFillable
        );
    }

    #[test]
    fn parse_external_solver_arg_user_balance_default() {
        let arg = "name|http://solver.com/|0x4242424242424242424242424242424242424242424242424242424242424242|false";
        let parsed = ExternalSolverArg::from_str(arg).unwrap();
        assert_eq!(parsed.user_balance_support, UserBalanceSupport::None);
    }
}
