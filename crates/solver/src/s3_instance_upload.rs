use {
    anyhow::{Context, Result},
    aws_credential_types::{provider::SharedCredentialsProvider, Credentials as AwsCredentials},
    aws_sdk_s3::{primitives::ByteStream, Client},
    aws_types::region::Region,
    flate2::{bufread::GzEncoder, Compression},
    model::auction::AuctionId,
    std::io::Read,
};

#[derive(Default)]
pub struct Config {
    pub bucket: String,
    /// Prepended to the auction id to form the final filename. Something like
    /// "staging/mainnet/quasimodo/". Should end with `/` if intended to be a
    /// folder.
    pub filename_prefix: String,
}

#[derive(Default)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

pub struct S3InstanceUploader {
    bucket: String,
    filename_prefix: String,
    client: Client,
}

impl S3InstanceUploader {
    pub async fn aws_credentials_from_cli_or_env(cli: Option<Credentials>) -> aws_types::SdkConfig {
        match cli {
            Some(args) => {
                aws_types::sdk_config::Builder::default()
                    .region(Region::new(args.region))
                    .credentials_provider(SharedCredentialsProvider::new(
                        AwsCredentials::from_keys(args.access_key_id, args.secret_access_key, None),
                    ))
                    .build()
            }
            // According to the AWS docs this is the recommended way to use the SDK. Unfortunately
            // we don't have a way to detect errors when loading from the environment.
            None => aws_config::load_from_env().await,
        }
    }

    pub fn new(config: Config, credentials: aws_types::SdkConfig) -> Self {
        Self {
            bucket: config.bucket,
            filename_prefix: config.filename_prefix,
            client: Client::new(&credentials),
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

    #[tokio::test]
    #[ignore]
    async fn print_filename() {
        let credentials = S3InstanceUploader::aws_credentials_from_cli_or_env(None).await;
        let uploader = S3InstanceUploader::new(
            Config {
                filename_prefix: "test/".to_string(),
                ..Default::default()
            },
            credentials,
        );
        let key = uploader.filename(11);
        println!("{key}");
    }

    #[tokio::test]
    #[ignore]
    async fn real_upload() {
        let config = Config {
            bucket: std::env::var("bucket").unwrap(),
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

        let credentials = S3InstanceUploader::aws_credentials_from_cli_or_env(None).await;
        let uploader = S3InstanceUploader::new(config, credentials);
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
