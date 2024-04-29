use {
    crate::{
        domain::liquidity::{self},
        infra::{api::Auction, solver::Config},
    },
    number::serialization::HexOrDecimalU256,
    primitive_types::U256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::sync::Arc,
    tracing::Instrument,
};

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuctionWithLiquidity {
    auction: Auction,
    liquidity: Vec<Liquidity>,
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Liquidity {
    id: usize,
    #[serde_as(as = "HexOrDecimalU256")]
    gas: U256,
    kind: Kind,
}

impl From<liquidity::Liquidity> for Liquidity {
    fn from(value: liquidity::Liquidity) -> Self {
        Self {
            id: value.id.into(),
            gas: value.gas.into(),
            kind: value.kind.into(),
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    UniswapV2,
    UniswapV3,
    BalancerV2Stable,
    BalancerV2Weighted,
    Swapr,
    ZeroEx,
}

impl From<liquidity::Kind> for Kind {
    fn from(value: liquidity::Kind) -> Self {
        match value {
            liquidity::Kind::UniswapV2(_) => Self::UniswapV2,
            liquidity::Kind::UniswapV3(_) => Self::UniswapV3,
            liquidity::Kind::BalancerV2Stable(_) => Self::BalancerV2Stable,
            liquidity::Kind::BalancerV2Weighted(_) => Self::BalancerV2Weighted,
            liquidity::Kind::Swapr(_) => Self::Swapr,
            liquidity::Kind::ZeroEx(_) => Self::ZeroEx,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct S3 {
    /// The s3_instance_upload_* arguments configure how auction instances
    /// should be uploaded to AWS S3.
    /// They must either all be set or all not set.
    pub bucket: String,

    /// Prepended to the auction id to form the final instance filename on S3.
    /// Something like "staging/mainnet/"
    pub prefix: String,
}

impl From<S3> for s3::Config {
    fn from(value: S3) -> Self {
        Self {
            bucket: value.bucket,
            filename_prefix: value.prefix,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Persistence {
    s3: Option<Arc<s3::Uploader>>,
}

impl Persistence {
    pub async fn build(config: &Config) -> Self {
        if let Some(s3) = &config.s3 {
            Self {
                s3: Some(Arc::new(s3::Uploader::new(s3.clone().into()).await)),
            }
        } else {
            Self { s3: None }
        }
    }

    /// Saves the given auction with liquidity with fire and forget mentality
    /// (non-blocking operation)
    pub fn archive_auction(&self, auction: Auction, liquidity: Vec<liquidity::Liquidity>) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        let auction_id = auction.id();
        let auction_with_liquidity = AuctionWithLiquidity {
            auction,
            liquidity: liquidity.into_iter().map(Into::into).collect(),
        };
        tokio::spawn(
            async move {
                match uploader
                    .upload(auction_id.to_string(), auction_with_liquidity)
                    .await
                {
                    Ok(key) => {
                        tracing::info!(?key, "uploaded auction to s3");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to upload auction to s3");
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}
