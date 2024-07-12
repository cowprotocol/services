use {
    sqlx::{
        encode::IsNull,
        error::BoxDynError,
        postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef},
        Decode,
        Encode,
        Postgres,
        Type,
    },
    std::fmt::{self, Debug, Formatter},
};

/// Wrapper type for fixed size byte arrays compatible with sqlx's Postgres
/// implementation.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct ByteArray<const N: usize>(pub [u8; N]);

impl<const N: usize> Debug for ByteArray<N> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl<const N: usize> Default for ByteArray<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

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
        self.0.encode(buf)
    }
}

impl<'de, const N: usize> serde::Deserialize<'de> for ByteArray<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ByteArrayVisitor<const N: usize>;

        impl<'de, const N: usize> serde::de::Visitor<'de> for ByteArrayVisitor<N> {
            type Value = ByteArray<N>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string with a '\\x' prefix")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let text = v.strip_prefix("\\x").ok_or_else(|| {
                    serde::de::Error::invalid_value(serde::de::Unexpected::Str(v), &self)
                })?;
                let mut bytes = [0u8; N];
                hex::decode_to_slice(text, &mut bytes).map_err(|_| {
                    serde::de::Error::invalid_value(serde::de::Unexpected::Str(v), &self)
                })?;
                Ok(ByteArray(bytes))
            }
        }

        deserializer.deserialize_str(ByteArrayVisitor)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde_json::json,
        sqlx::{Executor, PgPool, Row},
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_fixed_bytes() {
        const TABLE: &str = "fixed_bytes_test";
        let db = PgPool::connect("postgresql://").await.unwrap();
        db.execute(format!("CREATE TABLE IF NOT EXISTS {TABLE} (bytes bytea);").as_str())
            .await
            .unwrap();
        db.execute(format!("TRUNCATE {TABLE};").as_str())
            .await
            .unwrap();

        let data: ByteArray<3> = ByteArray([1, 2, 3]);
        sqlx::query(&format!("INSERT INTO {TABLE} (bytes) VALUES ($1);"))
            .bind(data)
            .execute(&db)
            .await
            .unwrap();
        let query = format!("SELECT * FROM {TABLE} LIMIT 1;");

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

    #[test]
    fn test_deserialize_byte_array() {
        // Valid deserialization
        let json_value = json!("\\x010203");
        let byte_array: ByteArray<3> = serde_json::from_value(json_value).unwrap();
        assert_eq!(byte_array, ByteArray([1, 2, 3]));

        // Invalid deserialization: wrong prefix
        let json_value = json!("010203");
        let result: Result<ByteArray<3>, _> = serde_json::from_value(json_value);
        assert!(result.is_err());

        // Invalid deserialization: wrong length
        let json_value = json!("\\x0102");
        let result: Result<ByteArray<3>, _> = serde_json::from_value(json_value);
        assert!(result.is_err());

        // Invalid deserialization: non-hex characters
        let json_value = json!("\\x01g203");
        let result: Result<ByteArray<3>, _> = serde_json::from_value(json_value);
        assert!(result.is_err());
    }
}
