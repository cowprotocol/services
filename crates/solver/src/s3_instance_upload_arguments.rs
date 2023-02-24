use {
    crate::s3_instance_upload::Config,
    shared::arguments::{display_option, display_secret_option},
};

#[derive(clap::Parser)]
pub struct S3UploadArguments {
    /// The s3_instance_upload_* arguments configure how quasimodo instances
    /// should be uploaded to AWS S3. They must either all be set or all not
    /// set. If they are set then every instance sent to Quasimodo as part
    /// of auction solving is also uploaded to S3.
    #[clap(long, env)]
    pub s3_instance_upload_region: Option<String>,

    #[clap(long, env)]
    pub s3_instance_upload_bucket: Option<String>,

    /// Prepended to the auction id to form the final instance filename on S3.
    /// Something like "staging/mainnet/quasimodo/". Should end with `/` if
    /// intended to be a folder.
    #[clap(long, env)]
    pub s3_instance_upload_filename_prefix: Option<String>,

    #[clap(long, env)]
    pub s3_instance_upload_access_key_id: Option<String>,

    #[clap(long, env)]
    pub s3_instance_upload_secret_access_key: Option<String>,
}

impl std::fmt::Display for S3UploadArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_option(
            f,
            "s3_instance_upload_region",
            &self.s3_instance_upload_region,
        )?;
        display_option(
            f,
            "s3_instance_upload_bucket",
            &self.s3_instance_upload_bucket,
        )?;
        display_option(
            f,
            "s3_instance_upload_filename_prefix",
            &self.s3_instance_upload_filename_prefix,
        )?;
        display_option(
            f,
            "s3_instance_upload_access_key_id",
            &self.s3_instance_upload_access_key_id,
        )?;
        display_secret_option(
            f,
            "s3_instance_upload_secret_access_key",
            &self.s3_instance_upload_secret_access_key,
        )?;
        Ok(())
    }
}

impl S3UploadArguments {
    pub fn into_config(self) -> anyhow::Result<Option<Config>> {
        let s3_args = &[
            &self.s3_instance_upload_region,
            &self.s3_instance_upload_bucket,
            &self.s3_instance_upload_filename_prefix,
            &self.s3_instance_upload_access_key_id,
            &self.s3_instance_upload_secret_access_key,
        ];
        let all_some = s3_args.iter().all(|arg| arg.is_some());
        let all_none = s3_args.iter().all(|arg| arg.is_none());
        anyhow::ensure!(
            all_some || all_none,
            "either set all s3_instance_upload arguments or none"
        );
        Ok(if all_some {
            Some(Config {
                region: self.s3_instance_upload_region.unwrap(),
                bucket: self.s3_instance_upload_bucket.unwrap(),
                access_key_id: self.s3_instance_upload_access_key_id.unwrap(),
                secret_access_key: self.s3_instance_upload_secret_access_key.unwrap(),
                filename_prefix: self.s3_instance_upload_filename_prefix.unwrap(),
            })
        } else {
            None
        })
    }
}
