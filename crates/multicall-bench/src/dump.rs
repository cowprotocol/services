//! Pulls the real working set out of a protocol database replica.

use {
    crate::fixture::{Fixture, Pair},
    alloy_primitives::Address,
    anyhow::{Context, Result, ensure},
    sqlx::{Row, postgres::PgConnectOptions},
    std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// The `OPEN_ORDERS` conditions from `database::orders::solvable_orders`,
/// reduced to the distinct `(owner, sell_token)` pairs the autopilot ends up
/// asking for balances of. Orders with pre-interactions and non-ERC20 sell
/// sources are excluded because those never reach the batched path.
const QUERY: &str = r#"
WITH live_orders AS MATERIALIZED (
    SELECT o.uid, o.owner, o.sell_token, o.kind, o.sell_amount, o.buy_amount
    FROM   orders o
    WHERE  o.cancellation_timestamp IS NULL
        AND o.true_valid_to >= $1
        AND o.sell_token_balance = 'erc20'
        AND NOT EXISTS (SELECT 1 FROM invalidations i WHERE i.order_uid = o.uid)
        AND NOT EXISTS (SELECT 1 FROM onchain_order_invalidations oi WHERE oi.uid = o.uid)
        AND NOT EXISTS (SELECT 1 FROM onchain_placed_orders op WHERE op.uid = o.uid AND op.placement_error IS NOT NULL)
        AND NOT EXISTS (SELECT 1 FROM ethflow_refunds r WHERE r.order_uid = o.uid)
        AND NOT EXISTS (SELECT 1 FROM interactions p WHERE p.order_uid = o.uid AND p.execution = 'pre')
),
trades_agg AS (
    SELECT t.order_uid,
           SUM(t.buy_amount)  AS sum_buy,
           SUM(t.sell_amount) AS sum_sell
    FROM   trades t
    JOIN   live_orders lo ON lo.uid = t.order_uid
    GROUP  BY t.order_uid
)
SELECT DISTINCT lo.owner, lo.sell_token
FROM   live_orders lo
LEFT   JOIN trades_agg ta ON ta.order_uid = lo.uid
WHERE  ((lo.kind = 'sell' AND COALESCE(ta.sum_sell, 0) < lo.sell_amount) OR
        (lo.kind = 'buy'  AND COALESCE(ta.sum_buy , 0) < lo.buy_amount))
LIMIT  $2
"#;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Network to dump, and the database name on the replica.
    #[clap(long, default_value = "mainnet")]
    network: String,

    /// Full connection string. Takes precedence over the `COW_DB_*`
    /// environment variables.
    #[clap(long, env = "BENCH_DATABASE_URL")]
    database_url: Option<String>,

    #[clap(long, env = "COW_DB_HOST")]
    db_host: Option<String>,

    #[clap(long, env = "COW_DB_PORT", default_value = "5432")]
    db_port: u16,

    #[clap(long, env = "COW_DB_USER")]
    db_user: Option<String>,

    #[clap(long, env = "COW_DB_PASSWORD", hide_env_values = true)]
    db_password: Option<String>,

    /// Cap on the number of pairs. Unlimited by default; the whole open-order
    /// set is what the autopilot actually reads every block.
    #[clap(long)]
    limit: Option<i64>,

    /// Defaults outside the repo so a forgotten flag cannot drop a 400 KB
    /// fixture into the working tree.
    #[clap(long, short, default_value = "/tmp/multicall-fixture.json")]
    output: PathBuf,
}

pub async fn run(args: Args) -> Result<()> {
    let options = match &args.database_url {
        Some(url) => url.parse().context("could not parse --database-url")?,
        None => PgConnectOptions::new()
            .host(
                args.db_host
                    .as_deref()
                    .context("need --database-url or COW_DB_HOST")?,
            )
            .port(args.db_port)
            .username(
                args.db_user
                    .as_deref()
                    .context("need --database-url or COW_DB_USER")?,
            )
            .password(
                args.db_password
                    .as_deref()
                    .context("need --database-url or COW_DB_PASSWORD")?,
            )
            .database(&args.network),
    };

    let mut db = sqlx::PgPool::connect_with(options)
        .await
        .context("could not connect to the database")?
        .acquire()
        .await?;

    // Fail fast instead of hanging on a bad plan.
    sqlx::query("SET statement_timeout = '120s'")
        .execute(&mut *db)
        .await?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let rows = sqlx::query(QUERY)
        .bind(i64::try_from(now)?)
        .bind(args.limit.unwrap_or(i64::MAX))
        .fetch_all(&mut *db)
        .await
        .context("open-order query failed")?;

    let pairs: Vec<Pair> = rows
        .iter()
        .map(|row| {
            let owner: Vec<u8> = row.try_get("owner")?;
            let token: Vec<u8> = row.try_get("sell_token")?;
            Ok(Pair {
                owner: Address::try_from(owner.as_slice())?,
                token: Address::try_from(token.as_slice())?,
            })
        })
        .collect::<Result<_>>()?;

    ensure!(!pairs.is_empty(), "no open orders found");

    let fixture = Fixture {
        network: args.network,
        dumped_at: format!("unix:{now}"),
        pairs,
    };
    let (tokens, owners) = fixture.diversity();
    fixture.store(&args.output)?;

    println!(
        "wrote {} pairs ({tokens} distinct tokens, {owners} distinct owners) to {}",
        fixture.pairs.len(),
        args.output.display(),
    );
    Ok(())
}
