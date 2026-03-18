use {
    crate::{DEFAULT_QUERY_TIMEOUT, GLOBAL_QUERY_TIMEOUT},
    sqlx::{Database, Error, Executor, IntoArguments, query::Query},
    std::time::Duration,
};

pub trait QueryTimeout<'q, DB, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q, DB>,
{
    /// Execute the query and return the total number of rows affected.
    async fn execute_with_timeout<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> Result<DB::QueryResult, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c, Database = DB>;
}

impl<'q, DB, A: Send> QueryTimeout<'q, DB, A> for Query<'q, DB, A>
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
        tokio::time::timeout(timeout, self.execute(executor))
            .await
            .map_err(|_| sqlx::Error::Io(std::io::Error::other("query timed out")))?
    }
}
