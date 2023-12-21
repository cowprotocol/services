use {anyhow::Result, s3::Config};

#[derive(clap::Parser, Debug, Clone)]
pub struct UploadArguments {
    #[clap(long, env)]
    /// The s3_instance_upload_* arguments configure how auction instances
    /// should be uploaded to AWS S3.
    /// They must either all be set or all not set.
    pub s3_instance_upload_bucket: Option<String>,

    /// Prepended to the auction id to form the final instance filename on S3.
    /// Something like "staging/mainnet/"
    #[clap(long, env)]
    pub s3_instance_upload_filename_prefix: Option<String>,
}

impl UploadArguments {
    pub fn into_config(self) -> Result<Option<Config>> {
        let s3_args = &[
            &self.s3_instance_upload_bucket,
            &self.s3_instance_upload_filename_prefix,
        ];
        let all_some = s3_args.iter().all(|arg| arg.is_some());
        let all_none = s3_args.iter().all(|arg| arg.is_none());
        anyhow::ensure!(
            all_some || all_none,
            "either set all s3_instance_upload bucket arguments or none"
        );
        Ok(if all_some {
            Some(Config {
                bucket: self.s3_instance_upload_bucket.unwrap(),
                filename_prefix: self.s3_instance_upload_filename_prefix.unwrap(),
            })
        } else {
            None
        })
    }
}
