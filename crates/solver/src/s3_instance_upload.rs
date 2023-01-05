use anyhow::Result;
use aws_sdk_s3::{types::ByteStream, Client, Credentials, Region};
use aws_types::credentials::SharedCredentialsProvider;
use chrono::{DateTime, Utc};
use model::auction::AuctionId;

#[derive(Default)]
pub struct Config {
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    /// Prepended to the auction id to form the final filename. Something like
    /// "staging/mainnet/quasimodo/". Should end with `/` if intended to be a folder.
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

    /// Upload the bytes (expected to represent a json encoded solver instance) to the configured S3
    /// bucket.
    ///
    /// The final filename is the configured prefix followed by `{current_date}/{auction_id}`.
    pub async fn upload_instance(&self, auction: AuctionId, value: Vec<u8>) -> Result<()> {
        let key = self.filename(chrono::offset::Utc::now(), auction);
        self.upload(key, value).await
    }

    fn filename(&self, now: DateTime<Utc>, auction: AuctionId) -> String {
        let date = now.format("%Y-%m-%d");
        format!("{}{date}/{auction}.json", self.filename_prefix)
    }

    async fn upload(&self, key: String, value: Vec<u8>) -> Result<()> {
        self.client
            .put_object()
            .bucket(self.bucket.clone())
            .key(key)
            .body(ByteStream::new(value.into()))
            .send()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn print_filename() {
        let uploader = S3InstanceUploader::new(Default::default());
        let key = uploader.filename(chrono::offset::Utc::now(), 11);
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

        let key = "test.txt".to_string();
        let value = format!("Hello {:?}", std::time::SystemTime::now());

        let uploader = S3InstanceUploader::new(config);
        uploader
            .upload(key.clone(), value.as_bytes().to_vec())
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
        let body = std::str::from_utf8(&body).unwrap();

        assert_eq!(value, body);
    }
}
