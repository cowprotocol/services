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

/// Writes the report document to `<dir>/<network>_<timestamp>.json`, creating
/// the directory if needed, and returns the path it wrote. `timestamp` is the
/// UTC run time in `YYYYMMDDThhmmssZ` form so files sort chronologically.
pub fn save_report(dir: &Path, network: &str, doc: &serde_json::Value) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("could not create report directory {}", dir.display()))?;
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("{network}_{timestamp}.json"));
    let contents = serde_json::to_string_pretty(doc)?;
    std::fs::write(&path, contents)
        .with_context(|| format!("could not write report to {}", path.display()))?;
    Ok(path)
}

/// A single completed `verify` run, recorded for history and progress.
pub struct VerifyRun {
    pub network: String,
    pub chain_id: u64,
    pub db_url: String,
    pub from_block: u64,
    pub to_block: u64,
    pub blocks_scanned: u64,
    pub mismatch_blocks: u64,
    pub mismatches: u64,
    /// A truncated run did not finish its range, so it must not advance the
    /// verified high-water mark.
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

    /// Records a run in the history and, unless it was truncated, advances the
    /// (chain, db) coverage extent to include the run's range.
    pub async fn record_run(&mut self, run: &VerifyRun) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO verify_runs (network, chain_id, db_url, from_block, to_block, \
             blocks_scanned, mismatch_blocks, mismatches, truncated, report_path, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.network)
        .bind(run.chain_id.cast_signed())
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

        if !run.truncated {
            // Widen the recorded coverage to span both the previous extent and
            // this run's range.
            sqlx::query(
                "INSERT INTO network_progress (chain_id, db_url, network, verified_from_block, \
                 verified_to_block, last_mismatches, last_report_path, updated_at) VALUES (?1, \
                 ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT (chain_id, db_url) DO UPDATE SET network \
                 = ?3, verified_from_block = min(verified_from_block, ?4), verified_to_block = \
                 max(verified_to_block, ?5), last_mismatches = ?6, last_report_path = ?7, \
                 updated_at = ?8",
            )
            .bind(run.chain_id.cast_signed())
            .bind(&run.db_url)
            .bind(&run.network)
            .bind(run.from_block.cast_signed())
            .bind(run.to_block.cast_signed())
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

    #[tokio::test]
    async fn save_report_writes_named_file() {
        let dir = tempfile::tempdir().unwrap();
        let doc = json!({ "network": "mainnet", "total": 0 });
        let path = save_report(dir.path(), "mainnet", &doc).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("mainnet_") && name.ends_with(".json"),
            "{name}"
        );
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written, doc);
    }

    #[tokio::test]
    async fn progress_high_water_advances_and_skips_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("progress.sqlite");
        let mut store = ProgressStore::open(&db).await.unwrap();

        store.record_run(&run(100, 200, 0, false)).await.unwrap();
        store.record_run(&run(200, 300, 2, false)).await.unwrap();
        // A truncated run must not advance coverage even though it reaches further.
        store.record_run(&run(300, 999, 0, true)).await.unwrap();

        let (from, to, runs): (i64, i64, i64) = sqlx::query_as(
            "SELECT verified_from_block, verified_to_block, (SELECT count(*) FROM verify_runs) \
             FROM network_progress WHERE chain_id = 1 AND db_url = 'postgres://prod'",
        )
        .fetch_one(&mut store.conn)
        .await
        .unwrap();

        assert_eq!(from, 100);
        assert_eq!(to, 300);
        assert_eq!(runs, 3);
    }
}
