use {
    crate::{DEFAULT_QUERY_TIMEOUT, GLOBAL_QUERY_TIMEOUT},
    sqlx::{
        Database,
        Error,
        Execute,
        Executor,
        IntoArguments,
        prelude::FromRow,
        query::{Query, QueryAs},
    },
};

pub trait QueryTimeoutExt<'q, DB, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
{
    async fn execute_with_timeout<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> Result<DB::QueryResult, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c, Database = DB>;
}

impl<'q, DB, A: Send> QueryTimeoutExt<'q, DB, A> for Query<'q, DB, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
{
    #[inline]
    async fn execute_with_timeout<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> Result<DB::QueryResult, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c, Database = DB>,
    {
        let timeout = *GLOBAL_QUERY_TIMEOUT
            .get()
            .unwrap_or_else(|| &DEFAULT_QUERY_TIMEOUT);
        let query = self.sql();

        tokio::time::timeout(timeout, self.execute(executor))
            .await
            .map_err(|_elapsed| {
                tracing::error!(%query, "query timed out (longer than {}s)", timeout.as_secs());
                sqlx::Error::Io(std::io::Error::other("query timed out"))
            })?
    }
}

pub trait QueryAsTimeoutExt<'q, DB, O, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
    O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    async fn fetch_all_with_timeout<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c, Database = DB>;
}

impl<'q, DB, O, A> QueryAsTimeoutExt<'q, DB, O, A> for QueryAs<'q, DB, O, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
    O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    #[inline]
    async fn fetch_all_with_timeout<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c, Database = DB>,
    {
        let timeout = *GLOBAL_QUERY_TIMEOUT
            .get()
            .unwrap_or_else(|| &DEFAULT_QUERY_TIMEOUT);
        let query = self.sql();

        tokio::time::timeout(timeout, self.fetch_all(executor))
            .await
            .map_err(|_elapsed| {
                tracing::error!(%query, "query timed out (longer than {}s)", timeout.as_secs());
                sqlx::Error::Io(std::io::Error::other("query timed out"))
            })?
    }
}
