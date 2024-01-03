use {crate::database::Postgres, std::sync::Arc};

pub mod auction;
pub mod quotes;

#[derive(Debug, Clone)]
pub struct Database {
    db: Arc<Postgres>,
}

impl Database {
    pub fn new(db: Arc<Postgres>) -> Self {
        Self { db }
    }
}
