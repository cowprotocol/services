//! Small abstraction over the AWS S3 SDK to upload arbitrary json object to S3.

use {
    anyhow::{anyhow, Context, Result},
    aws_sdk_s3::{primitives::ByteStream, Client},
    flate2::{bufread::GzEncoder, Compression},
    serde::Serialize,
    std::io::Read,
};

#[derive(Default)]
pub struct Config {
    pub bucket: String,
    /// Prepended to the the final filename for each uploaded object.
    pub filename_prefix: String,
}

#[derive(Clone)]
pub struct Uploader {
    bucket: String,
    filename_prefix: String,
    client: Client,
}

impl Uploader {
    pub async fn new(config: Config) -> Self {
        let uploader = Self {
            bucket: config.bucket,
            filename_prefix: config.filename_prefix,
            client: Client::new(&aws_config::from_env().load().await),
        };
        uploader.assert_credentials_are_usable().await;
        uploader
    }

    /// Upload the bytes json encoded to the configured S3 bucket. Returns the
    /// key under which the file can be queried
    pub async fn upload(&self, id: String, content: impl Serialize) -> Result<String> {
        let bytes = serde_json::to_vec(&content)?;
        let encoded = self.gzip(&bytes)?;
        let key = std::path::Path::new(&self.filename_prefix)
            .join(format!("{id}.json"))
            .to_str()
            .context(anyhow!("invalid path: {id}"))?
            .to_string();
        self.client
            .put_object()
            .bucket(self.bucket.clone())
            .key(key.clone())
            .body(ByteStream::new(encoded.into()))
            .content_encoding("gzip")
            .content_type("application/json")
            .send()
            .await?;
        Ok(key)
    }

    /// Uploads a small test file to verify that the credentials loaded from the
    /// environment allow uploads to S3.
    async fn assert_credentials_are_usable(&self) {
        const DOCS_URL: &str = "https://docs.rs/aws-config/latest/aws_config/default_provider/credentials/struct.DefaultCredentialsChain.html";
        self.upload(
            "test".into(),
            "test file to verify uploading capabilities".as_bytes(),
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "Could not upload test file to S3.\n Either disable uploads to S3 by removing the \
                 s3_instance_upload_* arguments.\n Or make sure your environment variables are \
                 set up to contain the correct AWS credentials.\n See {DOCS_URL} for more details \
                 on that. \n{err:?}"
            )
        });
    }

    /// Compresses the input bytes using Gzip.
    fn gzip(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(bytes, Compression::best());
        let mut encoded: Vec<u8> = Vec::with_capacity(bytes.len());
        encoder.read_to_end(&mut encoded).context("gzip encoding")?;
        Ok(encoded)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, flate2::read::GzDecoder, serde_json::json};

    // This test requires AWS credentials to be set via env variables.
    // See https://docs.rs/aws-config/latest/aws_config/default_provider/credentials/struct.DefaultCredentialsChain.html
    // to know which arguments are expected and in what precedence they
    // get loaded.
    #[tokio::test]
    #[ignore]
    async fn real_upload() {
        let config = Config {
            bucket: std::env::var("BUCKET").unwrap(),
            filename_prefix: "test/".to_string(),
        };

        // Upload a reasonable amount of data. This helps see the benefits of
        // compression.
        let value = serde_json::to_string(&json!({
            "content": include_str!("../../../README.md"),
            "timestamp": chrono::Utc::now().to_string(),
        }))
        .unwrap();

        let uploader = Uploader::new(config).await;
        let key = uploader
            .upload("test".to_string(), value.as_bytes())
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
