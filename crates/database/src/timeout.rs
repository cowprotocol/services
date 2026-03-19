//! Timeout wrappers for sqlx read queries.
//!
//! Provides extension traits that wrap sqlx's `fetch_*` methods with a
//! configurable global timeout ([`GLOBAL_QUERY_TIMEOUT`]). Only **read**
//! queries should use these wrappers — writes must not be cancelled
//! mid-flight as that could leave the database in an inconsistent state.
//!
//! The [`timeout_method_sig`] and [`timeout_method_impl`] macros generate
//! the trait signatures and implementations respectively, ensuring every
//! wrapper follows the exact same timeout-and-log pattern.

use {
    sqlx::{
        Database,
        Error,
        Execute,
        Executor,
        IntoArguments,
        prelude::FromRow,
        query::{QueryAs, QueryScalar},
    },
    std::{sync::OnceLock, time::Duration},
};

pub const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Process-wide query timeout override.
///
/// Must be initialized once at startup via [`init_global_query_timeout`].
/// If never set, [`DEFAULT_QUERY_TIMEOUT`] is used as a fallback.
static GLOBAL_QUERY_TIMEOUT: OnceLock<Duration> = OnceLock::new();

/// Initializes the global query timeout for all database read queries.
///
/// Must be called once at startup. Panics if called more than once.
pub fn init_global_query_timeout(timeout: Duration) {
    GLOBAL_QUERY_TIMEOUT
        .set(timeout)
        .expect("global query timeout already initialized");
}

fn query_timeout() -> std::time::Duration {
    *GLOBAL_QUERY_TIMEOUT.get().unwrap_or(&DEFAULT_QUERY_TIMEOUT)
}

fn timeout_error(query: &str, timeout: std::time::Duration) -> sqlx::Error {
    tracing::error!(%query, "query timed out (longer than {}s)", timeout.as_secs());
    sqlx::Error::Io(std::io::Error::other("query timed out"))
}

/// Generates a trait method signature for a `_with_timeout` wrapper.
macro_rules! timeout_method_sig {
    ($method:ident, $ret:ty) => {
        async fn $method<'e, 'c: 'e, E>(self, executor: E) -> Result<$ret, Error>
        where
            'q: 'e,
            A: 'e,
            E: Executor<'c, Database = DB>;
    };
}

/// Generates a trait method implementation that wraps the inner sqlx method
/// with a timeout.
macro_rules! timeout_method_impl {
    ($method:ident, $inner:ident, $ret:ty) => {
        #[inline]
        async fn $method<'e, 'c: 'e, E>(self, executor: E) -> Result<$ret, Error>
        where
            'q: 'e,
            A: 'e,
            E: Executor<'c, Database = DB>,
        {
            let timeout = query_timeout();
            let query = self.sql();
            tokio::time::timeout(timeout, self.$inner(executor))
                .await
                .map_err(|_elapsed| timeout_error(query, timeout))?
        }
    };
}

/// Timeout-guarded fetch methods for [`QueryAs`] (typed row queries).
pub trait QueryAsTimeoutExt<'q, DB, O, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
    O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    timeout_method_sig!(fetch_all_with_timeout, Vec<O>);

    timeout_method_sig!(fetch_one_with_timeout, O);

    timeout_method_sig!(fetch_optional_with_timeout, Option<O>);
}

impl<'q, DB, O, A> QueryAsTimeoutExt<'q, DB, O, A> for QueryAs<'q, DB, O, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
    O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    timeout_method_impl!(fetch_all_with_timeout, fetch_all, Vec<O>);

    timeout_method_impl!(fetch_one_with_timeout, fetch_one, O);

    timeout_method_impl!(fetch_optional_with_timeout, fetch_optional, Option<O>);
}

/// Timeout-guarded fetch methods for [`QueryScalar`] (single-column queries).
pub trait QueryScalarTimeoutExt<'q, DB, O, A>
where
    DB: Database,
    O: Send + Unpin,
    A: 'q + IntoArguments<'q, DB>,
    (O,): Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    timeout_method_sig!(fetch_one_with_timeout, O);

    timeout_method_sig!(fetch_optional_with_timeout, Option<O>);
}

impl<'q, DB, O, A> QueryScalarTimeoutExt<'q, DB, O, A> for QueryScalar<'q, DB, O, A>
where
    DB: Database,
    O: Send + Unpin,
    A: 'q + IntoArguments<'q, DB>,
    (O,): Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    timeout_method_impl!(fetch_one_with_timeout, fetch_one, O);

    timeout_method_impl!(fetch_optional_with_timeout, fetch_optional, Option<O>);
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        sqlx::{Connection, PgConnection},
    };

    /// Checks that the timeout is being applied and
    /// that it does not stop "regular" queries.
    #[tokio::test]
    #[ignore]
    async fn postgres_query_timeout() {
        let _ = GLOBAL_QUERY_TIMEOUT.set(Duration::from_millis(50));

        let mut db = PgConnection::connect("postgresql://").await.unwrap();

        let err = sqlx::query_scalar::<_, i32>("SELECT 1 FROM pg_sleep(1)")
            .fetch_one_with_timeout(&mut db)
            .await;
        assert!(err.is_err(), "expected timeout error");
        assert!(
            err.unwrap_err().to_string().contains("query timed out"),
            "expected 'query timed out' message"
        );

        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let result = sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one_with_timeout(&mut db)
            .await;
        assert_eq!(result.unwrap(), 1);
    }
}
