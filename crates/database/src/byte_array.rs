use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef},
    Decode, Encode, Postgres, Type,
};

/// Wrapper type for fixed size byte arrays compatible with sqlx's Postgres implementation.
#[derive(Clone, Copy, Debug)]
pub struct ByteArray<const N: usize>(pub [u8; N]);

impl<const N: usize> Default for ByteArray<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> PartialEq for ByteArray<N> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<const N: usize> Eq for ByteArray<N> {}

impl<const N: usize> Type<Postgres> for ByteArray<N> {
    fn type_info() -> PgTypeInfo {
        <[u8] as Type<Postgres>>::type_info()
    }
}

impl<const N: usize> PgHasArrayType for ByteArray<N> {
    fn array_type_info() -> PgTypeInfo {
        <[&[u8]] as Type<Postgres>>::type_info()
    }
}

impl<const N: usize> Decode<'_, Postgres> for ByteArray<N> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        let mut bytes = [0u8; N];
        match value.format() {
            // prepared query
            PgValueFormat::Binary => {
                bytes = value.as_bytes()?.try_into()?;
            }
            // unprepared raw query
            PgValueFormat::Text => {
                let text = value
                    .as_bytes()?
                    .strip_prefix(b"\\x")
                    .ok_or("text does not start with \\x")?;
                hex::decode_to_slice(text, &mut bytes)?
            }
        };
        Ok(Self(bytes))
    }
}

impl<const N: usize> Encode<'_, Postgres> for ByteArray<N> {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        <&[u8] as Encode<Postgres>>::encode(&self.0, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{Executor, PgPool, Row};

    #[tokio::test]
    #[ignore]
    async fn postgres_fixed_bytes() {
        const TABLE: &str = "fixed_bytes_test";
        let db = PgPool::connect("postgresql://").await.unwrap();
        db.execute(format!("CREATE TABLE IF NOT EXISTS {} (bytes bytea);", TABLE).as_str())
            .await
            .unwrap();
        db.execute(format!("TRUNCATE {};", TABLE).as_str())
            .await
            .unwrap();

        let data: ByteArray<3> = ByteArray([1, 2, 3]);
        sqlx::query(&format!("INSERT INTO {} (bytes) VALUES ($1);", TABLE))
            .bind(&data)
            .execute(&db)
            .await
            .unwrap();
        let query = format!("SELECT * FROM {} LIMIT 1;", TABLE);

        // unprepared raw query
        let row = db.fetch_one(query.as_str()).await.unwrap();
        let data_: ByteArray<3> = row.try_get(0).unwrap();
        assert_eq!(data.0, data_.0);

        // prepared query
        let data_: ByteArray<3> = sqlx::query_scalar(&query).fetch_one(&db).await.unwrap();
        assert_eq!(data.0, data_.0);

        // wrong size error, raw query
        let row = db.fetch_one(query.as_str()).await.unwrap();
        let result = row.try_get::<ByteArray<0>, _>(0);
        assert!(result.is_err());

        // wrong size error, prepared
        let result = sqlx::query_scalar::<_, ByteArray<4>>(&query)
            .fetch_one(&db)
            .await;
        assert!(result.is_err());
    }
}
