//! Persistence for `verify` runs: saving each final report to a JSON file and
//! tracking how far each network has been verified in a local SQLite database.
//!
//! Progress is keyed by (chain_id, db_url) rather than by chain alone: the same
//! chain is indexed into different databases per environment (staging, prod,
//! …), and each of those is verified independently, so they must not clobber
//! each other's high-water mark.

use {
    anyhow::{Context, Result},
    chain::Chain,
    chrono::Utc,
    sqlx::{ConnectOptions, SqliteConnection, sqlite::SqliteConnectOptions},
    std::path::{Path, PathBuf},
};

/// The canonical CoW slug for a chain id, or `chain-<id>` for one we don't
/// know.
pub fn network_name(chain_id: u64) -> String {
    Chain::try_from(chain_id)
        .map(|c| c.as_str().to_owned())
        .unwrap_or_else(|_| format!("chain-{chain_id}"))
}

/// Reduces an RPC URL to scheme and host for recording which data source a run
/// used. The path, query and userinfo are dropped because that is where
/// providers embed API keys (e.g. `https://eth-mainnet.g.alchemy.com/v2/KEY`).
pub fn sanitize_rpc_url(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, url),
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit('@').next().unwrap_or(host);
    match scheme {
        Some(scheme) => format!("{scheme}://{host}"),
        None => host.to_owned(),
    }
}

/// Writes the report document to `<dir>/<network>-<timestamp>.json`, creating
/// the directory if needed, and returns the path it wrote. `timestamp` is the
/// UTC run time in `YYYYMMDDThhmmssZ` form so files sort chronologically.
pub fn save_report(dir: &Path, network: &str, doc: &serde_json::Value) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("could not create report directory {}", dir.display()))?;
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("{network}-{timestamp}.json"));
    let contents = serde_json::to_string_pretty(doc)?;
    std::fs::write(&path, contents)
        .with_context(|| format!("could not write report to {}", path.display()))?;
    Ok(path)
}

/// A single completed `verify` run, recorded for history and progress.
pub struct VerifyRun {
    pub network: String,
    pub chain_id: u64,
    /// Sanitized (scheme and host only) so a mismatch report can be traced
    /// back to the node that produced its chain view.
    pub rpc_url: String,
    pub db_url: String,
    pub from_block: u64,
    pub to_block: u64,
    pub blocks_scanned: u64,
    pub mismatch_blocks: u64,
    pub mismatches: u64,
    /// Whether the run stopped before its intended `to_block` (a failed chunk,
    /// a DB error, or the mismatch cap). Recorded in the run history; coverage
    /// still advances by the blocks actually scanned, since the scan is
    /// contiguous.
    pub truncated: bool,
    pub report_path: Option<String>,
}

/// SQLite store tracking verification progress across runs and environments.
pub struct ProgressStore {
    conn: SqliteConnection,
}

impl ProgressStore {
    /// Opens (creating if absent) the SQLite database at `path` and ensures the
    /// schema exists.
    pub async fn open(path: &Path) -> Result<Self> {
        let mut conn = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .connect()
            .await
            .with_context(|| format!("could not open progress database {}", path.display()))?;

        // `verify_runs` is an append-only history; `network_progress` is the
        // per-(chain, db) high-water mark of contiguous coverage extent.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS verify_runs (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                network        TEXT    NOT NULL,
                chain_id       INTEGER NOT NULL,
                db_url         TEXT    NOT NULL,
                from_block     INTEGER NOT NULL,
                to_block       INTEGER NOT NULL,
                blocks_scanned INTEGER NOT NULL,
                mismatch_blocks INTEGER NOT NULL,
                mismatches     INTEGER NOT NULL,
                truncated      INTEGER NOT NULL,
                report_path    TEXT,
                created_at     TEXT    NOT NULL
            )",
        )
        .execute(&mut conn)
        .await
        .context("could not create verify_runs table")?;

        // Older databases predate the rpc_url column; add it in place.
        let (has_rpc_url,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM pragma_table_info('verify_runs') WHERE name = 'rpc_url'",
        )
        .fetch_one(&mut conn)
        .await
        .context("could not inspect the verify_runs schema")?;
        if has_rpc_url == 0 {
            sqlx::query("ALTER TABLE verify_runs ADD COLUMN rpc_url TEXT")
                .execute(&mut conn)
                .await
                .context("could not add the rpc_url column")?;
        }

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS network_progress (
                chain_id            INTEGER NOT NULL,
                db_url              TEXT    NOT NULL,
                network             TEXT    NOT NULL,
                verified_from_block INTEGER NOT NULL,
                verified_to_block   INTEGER NOT NULL,
                last_mismatches     INTEGER NOT NULL,
                last_report_path    TEXT,
                updated_at          TEXT    NOT NULL,
                PRIMARY KEY (chain_id, db_url)
            )",
        )
        .execute(&mut conn)
        .await
        .context("could not create network_progress table")?;

        Ok(Self { conn })
    }

    /// The last block verified for `(chain_id, db_url)`, i.e. the upper end of
    /// the recorded contiguous coverage, or `None` if this environment has no
    /// prior run. A scan can resume from the block after it.
    pub async fn verified_to_block(&mut self, chain_id: u64, db_url: &str) -> Result<Option<u64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT verified_to_block FROM network_progress WHERE chain_id = ?1 AND db_url = ?2",
        )
        .bind(chain_id.cast_signed())
        .bind(db_url)
        .fetch_optional(&mut self.conn)
        .await
        .context("could not read recorded progress")?;
        Ok(row.map(|(to,)| to.cast_unsigned()))
    }

    /// Records a run in the history and advances the (chain, db) coverage
    /// extent to the blocks it actually verified. A forward scan is contiguous,
    /// so even a run that stopped early still extends coverage up to its last
    /// scanned block.
    pub async fn record_run(&mut self, run: &VerifyRun) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO verify_runs (network, chain_id, rpc_url, db_url, from_block, to_block, \
             blocks_scanned, mismatch_blocks, mismatches, truncated, report_path, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.network)
        .bind(run.chain_id.cast_signed())
        .bind(&run.rpc_url)
        .bind(&run.db_url)
        .bind(run.from_block.cast_signed())
        .bind(run.to_block.cast_signed())
        .bind(run.blocks_scanned.cast_signed())
        .bind(run.mismatch_blocks.cast_signed())
        .bind(run.mismatches.cast_signed())
        .bind(run.truncated)
        .bind(&run.report_path)
        .bind(&now)
        .execute(&mut self.conn)
        .await
        .context("could not insert verify run")?;

        // The scan verifies blocks in ascending order, so even a run that
        // stopped early (truncated, a failed chunk, the mismatch cap) has
        // contiguously verified [from_block, from_block + blocks_scanned - 1].
        // Advance coverage to that actual extent — not the intended to_block —
        // so an interrupted long scan lets the next run resume where it stopped
        // instead of restarting. A run that scanned nothing changes nothing.
        //
        // The extent is a single interval, so it may only absorb a range that
        // overlaps or touches it; a min/max hull over a disjoint range would
        // claim the gap between them was verified when it never was.
        if run.blocks_scanned > 0 {
            let existing: Option<(i64, i64)> = sqlx::query_as(
                "SELECT verified_from_block, verified_to_block FROM network_progress WHERE \
                 chain_id = ?1 AND db_url = ?2",
            )
            .bind(run.chain_id.cast_signed())
            .bind(&run.db_url)
            .fetch_optional(&mut self.conn)
            .await
            .context("could not read network progress")?;

            let (from, to) = (
                run.from_block.cast_signed(),
                (run.from_block + run.blocks_scanned - 1).cast_signed(),
            );
            let (extent_from, extent_to) = match existing {
                Some((old_from, old_to)) if from <= old_to + 1 && old_from <= to + 1 => {
                    (old_from.min(from), old_to.max(to))
                }
                Some((old_from, old_to)) => {
                    tracing::warn!(
                        old_from,
                        old_to,
                        from,
                        to,
                        "run range is disjoint from the recorded coverage; replacing the extent \
                         with the higher range (the store tracks one contiguous interval)"
                    );
                    if to > old_to {
                        (from, to)
                    } else {
                        (old_from, old_to)
                    }
                }
                None => (from, to),
            };

            sqlx::query(
                "INSERT INTO network_progress (chain_id, db_url, network, verified_from_block, \
                 verified_to_block, last_mismatches, last_report_path, updated_at) VALUES (?1, \
                 ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT (chain_id, db_url) DO UPDATE SET network \
                 = ?3, verified_from_block = ?4, verified_to_block = ?5, last_mismatches = ?6, \
                 last_report_path = ?7, updated_at = ?8",
            )
            .bind(run.chain_id.cast_signed())
            .bind(&run.db_url)
            .bind(&run.network)
            .bind(extent_from)
            .bind(extent_to)
            .bind(run.mismatches.cast_signed())
            .bind(&run.report_path)
            .bind(&now)
            .execute(&mut self.conn)
            .await
            .context("could not update network progress")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn run(from: u64, to: u64, mismatches: u64, truncated: bool) -> VerifyRun {
        VerifyRun {
            network: "mainnet".into(),
            chain_id: 1,
            rpc_url: "https://rpc.example.com".into(),
            db_url: "postgres://prod".into(),
            from_block: from,
            to_block: to,
            blocks_scanned: to - from + 1,
            mismatch_blocks: mismatches,
            mismatches,
            truncated,
            report_path: Some("reports/mainnet_x.json".into()),
        }
    }

    #[test]
    fn sanitize_rpc_url_strips_credentials() {
        for (url, expected) in [
            (
                "https://eth-mainnet.g.alchemy.com/v2/secret-key",
                "https://eth-mainnet.g.alchemy.com",
            ),
            (
                "https://user:pass@node.example.com:8545/path?apikey=k",
                "https://node.example.com:8545",
            ),
            ("wss://rpc.gnosischain.com/wss", "wss://rpc.gnosischain.com"),
            ("localhost:8545", "localhost:8545"),
        ] {
            assert_eq!(sanitize_rpc_url(url), expected);
        }
    }

    #[tokio::test]
    async fn save_report_writes_named_file() {
        let dir = tempfile::tempdir().unwrap();
        let doc = json!({ "network": "mainnet", "total": 0 });
        let path = save_report(dir.path(), "mainnet", &doc).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("mainnet-") && name.ends_with(".json"),
            "{name}"
        );
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written, doc);
    }

    #[tokio::test]
    async fn progress_advances_by_blocks_actually_scanned() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("progress.sqlite");
        let mut store = ProgressStore::open(&db).await.unwrap();

        store.record_run(&run(100, 200, 0, false)).await.unwrap();
        store.record_run(&run(200, 300, 2, false)).await.unwrap();
        // A truncated run still verified its range contiguously up to the last
        // block it scanned, so coverage advances to there — not its intended
        // to_block (999), and not nowhere.
        let mut truncated = run(300, 999, 0, true);
        truncated.blocks_scanned = 100; // scanned 300..=399 before stopping
        store.record_run(&truncated).await.unwrap();

        let (from, to, runs): (i64, i64, i64) = sqlx::query_as(
            "SELECT verified_from_block, verified_to_block, (SELECT count(*) FROM verify_runs) \
             FROM network_progress WHERE chain_id = 1 AND db_url = 'postgres://prod'",
        )
        .fetch_one(&mut store.conn)
        .await
        .unwrap();

        assert_eq!(from, 100);
        assert_eq!(to, 399);
        assert_eq!(runs, 3);
    }

    #[tokio::test]
    async fn verified_to_block_reads_back_the_high_water_mark() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("progress.sqlite");
        let mut store = ProgressStore::open(&db).await.unwrap();

        // No prior run: nothing to resume after.
        assert_eq!(
            store.verified_to_block(1, "postgres://prod").await.unwrap(),
            None
        );

        store.record_run(&run(100, 200, 0, false)).await.unwrap();
        assert_eq!(
            store.verified_to_block(1, "postgres://prod").await.unwrap(),
            Some(200)
        );
        // A run that scanned nothing (immediate failure) doesn't move the mark.
        let mut nothing = run(201, 999, 0, true);
        nothing.blocks_scanned = 0;
        store.record_run(&nothing).await.unwrap();
        assert_eq!(
            store.verified_to_block(1, "postgres://prod").await.unwrap(),
            Some(200)
        );
        // Scoped per (chain, db): another environment has no progress here.
        assert_eq!(
            store
                .verified_to_block(1, "postgres://staging")
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn progress_does_not_hull_over_disjoint_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("progress.sqlite");
        let mut store = ProgressStore::open(&db).await.unwrap();

        store.record_run(&run(100, 200, 0, false)).await.unwrap();
        // Disjoint higher range: the extent must move, not stretch over the
        // unverified gap 201..499.
        store.record_run(&run(500, 600, 0, false)).await.unwrap();

        let (from, to): (i64, i64) = sqlx::query_as(
            "SELECT verified_from_block, verified_to_block FROM network_progress WHERE chain_id = \
             1 AND db_url = 'postgres://prod'",
        )
        .fetch_one(&mut store.conn)
        .await
        .unwrap();
        assert_eq!((from, to), (500, 600));

        // A disjoint lower range must not shrink the recorded coverage.
        store.record_run(&run(0, 50, 0, false)).await.unwrap();
        let (from, to): (i64, i64) = sqlx::query_as(
            "SELECT verified_from_block, verified_to_block FROM network_progress WHERE chain_id = \
             1 AND db_url = 'postgres://prod'",
        )
        .fetch_one(&mut store.conn)
        .await
        .unwrap();
        assert_eq!((from, to), (500, 600));
    }
}
