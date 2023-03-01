use {
    anyhow::{Context, Result},
    aws_sdk_s3::{types::ByteStream, Client, Credentials, Region},
    aws_types::credentials::SharedCredentialsProvider,
    flate2::{bufread::GzEncoder, Compression},
    model::auction::AuctionId,
    std::io::Read,
};

#[derive(Default)]
pub struct Config {
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    /// Prepended to the auction id to form the final filename. Something like
    /// "staging/mainnet/quasimodo/". Should end with `/` if intended to be a
    /// folder.
    pub filename_prefix: String,
}

pub struct S3InstanceUploader {
    bucket: String,
    filename_prefix: String,
    client: Client,
}

impl S3InstanceUploader {
    pub fn new(config: Config) -> Self {
        let aws_config = aws_types::sdk_config::Builder::default()
            .region(Region::new(config.region))
            .credentials_provider(SharedCredentialsProvider::new(Credentials::from_keys(
                config.access_key_id,
                config.secret_access_key,
                None,
            )))
            .build();
        Self {
            bucket: config.bucket,
            filename_prefix: config.filename_prefix,
            client: Client::new(&aws_config),
        }
    }

    /// Upload the bytes (expected to represent a json encoded solver instance)
    /// to the configured S3 bucket.
    ///
    /// The final filename is the configured prefix followed by
    /// `{auction_id}.json.gzip`.
    pub async fn upload_instance(&self, auction: AuctionId, value: &[u8]) -> Result<()> {
        self.upload(self.filename(auction), value).await
    }

    /// Compresses the input bytes using Gzip.
    fn gzip(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(bytes, Compression::best());
        let mut encoded: Vec<u8> = Vec::with_capacity(bytes.len());
        encoder.read_to_end(&mut encoded).context("gzip encoding")?;
        Ok(encoded)
    }

    fn filename(&self, auction: AuctionId) -> String {
        format!("{}{auction}.json", self.filename_prefix)
    }

    async fn upload(&self, key: String, bytes: &[u8]) -> Result<()> {
        let encoded = self.gzip(bytes)?;
        self.client
            .put_object()
            .bucket(self.bucket.clone())
            .key(key)
            .body(ByteStream::new(encoded.into()))
            .content_encoding("gzip")
            .content_type("application/json")
            .send()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, flate2::read::GzDecoder, serde_json::json};

    #[test]
    #[ignore]
    fn print_filename() {
        let uploader = S3InstanceUploader::new(Config {
            filename_prefix: "test/".to_string(),
            ..Default::default()
        });
        let key = uploader.filename(11);
        println!("{key}");
    }

    #[tokio::test]
    #[ignore]
    async fn real_upload() {
        let config = Config {
            region: "eu-central-1".to_string(),
            bucket: std::env::var("bucket").unwrap(),
            access_key_id: std::env::var("access_key_id").unwrap(),
            secret_access_key: std::env::var("secret_access_key").unwrap(),
            filename_prefix: "".to_string(),
        };

        let key = "test.json".to_string();
        // Upload a reasonable amount of data. This helps see the benefits of
        // compression.
        let value = serde_json::to_string(&json!({
            "content": include_str!("../../../README.md"),
            "timestamp": chrono::Utc::now(),
        }))
        .unwrap();

        let uploader = S3InstanceUploader::new(config);
        uploader
            .upload(key.clone(), value.as_bytes())
            .await
            .unwrap();

        let get_object = uploader
            .client
            .get_object()
            .bucket(uploader.bucket)
            .key(key)
            .send()
            .await
            .unwrap();
        let body = get_object.body.collect().await.unwrap().to_vec();

        let mut decoder = GzDecoder::new(body.as_slice());
        let mut decoded = String::new();
        decoder.read_to_string(&mut decoded).unwrap();

        assert_eq!(value, decoded);
    }
}
