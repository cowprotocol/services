use sqlx::PgPool;

pub mod bad_tokens;

#[derive(Clone)]
pub struct Postgres {
    pub pool: PgPool,
}

impl Postgres {
    pub async fn new(url: &str) -> sqlx::Result<Self> {
        Ok(Self {
            pool: PgPool::connect(url).await?,
        })
    }
}
