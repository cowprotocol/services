//! Liquorice is an RFQ (Request for Quote) liquidity provider that
//! aggregates quotes from PMMs (Private Market Makers) and
//! provides an HTTP API for notifying PMMs when their quote
//! is used in CoW settlement.
//!
//! For more information on the HTTP API, consult:
//! <https://liquorice.gitbook.io/liquorice-docs>

use {
    crate::{
        domain::{
            competition::{solution, solution::Settlement},
            eth,
        },
        infra::{
            self,
            notify::liquidity_sources::{
                LiquiditySourcesNotifying,
                liquorice::{self},
            },
        },
    },
    anyhow::{Context, Result, anyhow},
    chrono::Utc,
    contracts::ILiquoriceSettlement,
    ethabi::Token,
    ethcontract::common::FunctionExt,
    std::collections::HashSet,
};

const NOTIFICATION_SOURCE: &str = "cow_protocol";
const DRIVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Notifier {
    /// Liquorice API client
    client: liquorice::Client,
    /// Address of the Liquorice settlement contract is used to
    /// find relevant interactions in CoW settlement contract
    settlement_contract_address: eth::Address,
}

impl Notifier {
    pub fn new(
        config: &infra::notify::liquidity_sources::config::Liquorice,
        chain: chain::Chain,
    ) -> Result<Self> {
        let settlement_contract_address = ILiquoriceSettlement::raw_contract()
            .networks
            .get(chain.id().to_string().as_str())
            .map(|network| network.address.into())
            .ok_or(anyhow!("Liquorice settlement contract not found"))?;

        Ok(Self {
            client: liquorice::Client::new(
                reqwest::ClientBuilder::default(),
                config.base_url.clone(),
                config.api_key.clone(),
                config.http_timeout,
            )?,
            settlement_contract_address,
        })
    }

    /// Extracts Liquorice maker RFQ IDs from the settlement interactions.
    /// <https://liquorice.gitbook.io/liquorice-docs/for-market-makers/basic-market-making-api#id-3.-receiving-rfq>
    fn extract_rfq_ids_from_settlement(&self, settlement: &Settlement) -> HashSet<String> {
        // Aggregate all interactions from the settlement
        settlement
            .pre_interactions()
            .iter()
            .filter_map(|interaction| {
                extract_rfq_id_from_interaction(interaction, self.settlement_contract_address)
            })
            .chain(
                settlement
                    .interactions()
                    .iter()
                    .filter_map(|interaction| match interaction {
                        solution::Interaction::Custom(custom) => extract_rfq_id_from_interaction(
                            &eth::Interaction {
                                target: custom.target.into(),
                                value: custom.value,
                                call_data: custom.call_data.clone(),
                            },
                            self.settlement_contract_address,
                        ),
                        solution::Interaction::Liquidity(_) => None,
                    }),
            )
            .chain(
                settlement
                    .post_interactions()
                    .iter()
                    .filter_map(|interaction| {
                        extract_rfq_id_from_interaction(
                            interaction,
                            self.settlement_contract_address,
                        )
                    }),
            )
            .collect()
    }
}

#[async_trait::async_trait]
impl LiquiditySourcesNotifying for Notifier {
    async fn settlement(&self, settlement: &Settlement) -> Result<()> {
        let rfq_ids = self.extract_rfq_ids_from_settlement(settlement);

        use liquorice::client::request::v1::intent_origin::notification::post::{
            Content,
            Metadata,
            Request,
            Settle,
        };

        let _ = self
            .client
            .send_request(Request {
                source: NOTIFICATION_SOURCE.to_string(),
                timestamp: Utc::now(),
                metadata: Metadata {
                    driver_version: DRIVER_VERSION.to_string(),
                },
                content: Content::Settle(Settle {
                    auction_id: settlement.auction_id.0,
                    rfq_ids,
                }),
            })
            .await
            .context("request error")?;

        Ok(())
    }
}

/// Extracts Liquorice RFQ ID from CoW interaction.
/// RFQ ID extracted from the calldata corresponding to the
/// `settleSingle` function of the LiquoriceSettlement contract <https://etherscan.io/address/0xaca684a3f64e0eae4812b734e3f8f205d3eed167#code#F6#L1>
fn extract_rfq_id_from_interaction(
    interaction: &eth::Interaction,
    settlement_contract_address: eth::Address,
) -> Option<String> {
    if interaction.target != settlement_contract_address {
        return None;
    }

    // Decode the calldata using the Liquorice settlement contract ABI
    let Some(tokens) = ({
        let settle_single_function = ILiquoriceSettlement::raw_contract()
            .interface
            .abi
            .function("settleSingle")
            .unwrap();

        interaction
            .call_data
            .0
            .strip_prefix(&settle_single_function.selector())
            .and_then(|input| settle_single_function.decode_input(input).ok())
    }) else {
        return None;
    };

    // Token at index 1 corresponds to `Single` order
    // <https://etherscan.io/address/0xaca684a3f64e0eae4812b734e3f8f205d3eed167#code#F6#L85>
    tokens.get(1).and_then(|token| match token {
        Token::Tuple(tokens) => {
            // Token at index 0 corresponds to `rfqId` field
            // <https://etherscan.io/address/0xaca684a3f64e0eae4812b734e3f8f205d3eed167#code#F6#L42>
            tokens.first().and_then(|token| match token {
                Token::String(rfq_id) => Some(rfq_id.clone()),
                _ => None,
            })
        }
        _ => None,
    })
}
