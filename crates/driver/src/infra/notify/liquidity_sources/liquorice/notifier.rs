//! Liquorice is an RFQ (Request for Quote) liquidity provider that
//! aggregates quotes from PMMs (Private Market Makers) and
//! provides an HTTP API for notifying PMMs when their quote
//! is used in CoW settlement.
//!
//! For more information on the HTTP API, consult:
//! <https://liquorice.gitbook.io/liquorice-docs>

use {
    crate::{
        domain::competition::solution::Settlement,
        infra::{
            self,
            notify::liquidity_sources::{
                LiquiditySourceNotifying,
                liquorice::{self, client::request::v1::intent_origin::notification},
            },
        },
    },
    alloy::primitives::Address,
    anyhow::{Context, Result, anyhow},
    chrono::Utc,
    contracts::alloy::LiquoriceSettlement,
};

const NOTIFICATION_SOURCE: &str = "cow_protocol";
const DRIVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Notifier {
    /// Liquorice API client
    client: liquorice::Client,
    /// Address of the Liquorice settlement contract is used to
    /// find relevant interactions in CoW settlement contract
    liquorice_settlement_contract_address: Address,
}

impl Notifier {
    pub fn new(
        config: &infra::notify::liquidity_sources::config::Liquorice,
        chain: chain::Chain,
    ) -> Result<Self> {
        let liquorice_settlement_contract_address =
            LiquoriceSettlement::deployment_address(&chain.id())
                .ok_or(anyhow!("Liquorice settlement contract not found"))?;

        Ok(Self {
            client: liquorice::Client::new(
                reqwest::ClientBuilder::default(),
                config.base_url.clone(),
                config.api_key.clone(),
                config.http_timeout,
            )?,
            liquorice_settlement_contract_address,
        })
    }
}

#[async_trait::async_trait]
impl LiquiditySourceNotifying for Notifier {
    async fn settlement(&self, settlement: &Settlement) -> Result<()> {
        let rfq_ids = utils::extract_rfq_ids_from_settlement(
            settlement,
            self.liquorice_settlement_contract_address,
        );

        self.client
            .send_request(notification::post::Request {
                source: NOTIFICATION_SOURCE.to_string(),
                timestamp: Utc::now(),
                metadata: notification::post::Metadata {
                    driver_version: DRIVER_VERSION.to_string(),
                },
                content: notification::post::Content::Settle(notification::post::Settle {
                    auction_id: settlement.auction_id.0,
                    rfq_ids,
                }),
            })
            .await
            .context("request error")?;

        Ok(())
    }
}

mod utils {
    use {
        crate::domain::competition::solution::{self, Settlement},
        alloy::{primitives::Address, sol_types::SolCall},
        contracts::alloy::LiquoriceSettlement,
        shared::domain::eth,
        std::collections::HashSet,
    };

    /// Extracts Liquorice maker RFQ IDs from the settlement interactions.
    /// <https://liquorice.gitbook.io/liquorice-docs/for-market-makers/basic-market-making-api#id-3.-receiving-rfq>
    pub fn extract_rfq_ids_from_settlement(
        settlement: &Settlement,
        liquorice_settlement_contract_address: Address,
    ) -> HashSet<String> {
        // Aggregate all interactions from the settlement and extract RFQ ID from each
        settlement
            .pre_interactions()
            .iter()
            .filter_map(|interaction| {
                extract_rfq_id_from_interaction(interaction, liquorice_settlement_contract_address)
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
                            liquorice_settlement_contract_address,
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
                            liquorice_settlement_contract_address,
                        )
                    }),
            )
            .collect()
    }

    /// Extracts Liquorice maker RFQ ID from CoW interaction.
    /// RFQ ID is extracted from the calldata corresponding to the
    /// `settleSingle` function of the LiquoriceSettlement contract
    /// <https://etherscan.io/address/0x0448633eb8b0a42efed924c42069e0dcf08fb552#code#F8#L83>
    pub fn extract_rfq_id_from_interaction(
        interaction: &eth::Interaction,
        liquorice_settlement_contract_address: Address,
    ) -> Option<String> {
        if interaction.target != liquorice_settlement_contract_address {
            return None;
        }

        // Decode the calldata using the Liquorice settlement contract ABI
        let input = LiquoriceSettlement::LiquoriceSettlement::settleSingleCall::abi_decode(
            &interaction.call_data.0,
        )
        .ok()?;

        Some(input._order.rfqId)
    }

    #[cfg(test)]
    mod tests {
        use {
            crate::infra::notify::liquidity_sources::liquorice::notifier::utils::extract_rfq_id_from_interaction,
            alloy::primitives::{Address, Bytes},
            shared::domain::eth,
        };

        #[test]
        fn test_extract_rfq_id_from_valid_settle_single_call() {
            let calldata = const_hex::decode("9935c868000000000000000000000000b10b9c690a681b6285c2e2df7734f9d729c5c4d500000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000260000000000000000000000000000000000000000000000000000000001dcd65000000000000000000000000000000000000000000000000000000000000000340000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000001dcd6500000000000000000000000000000000000000000000000000000000001dcd650000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000068a76fd0000000000000000000000000b10b9c690a681b6285c2e2df7734f9d729c5c4d5000000000000000000000000000000000000000000000000000000000000002463393964326533662d373032622d343963392d386262382d343337373537373066326633000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000041d44f6881d24cd3ad94561d1aad5e220929181cf728f89620629e0efbe3daa9833b9f2967bf37381f56b41266327a3804c92204f30a2d660cae8b42cd6f7d9b701c000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000041000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap();

            let liquorice_settlement_address = Address::random();
            let rfq_id = extract_rfq_id_from_interaction(
                &eth::Interaction {
                    target: liquorice_settlement_address,
                    call_data: calldata.into(),
                    value: 0.into(),
                },
                liquorice_settlement_address,
            )
            .unwrap();
            assert_eq!(rfq_id, "c99d2e3f-702b-49c9-8bb8-43775770f2f3".to_string());
        }

        #[test]
        fn test_returns_none_for_arbitrary_call() {
            let liquorice_settlement_address = Address::random();
            let rfq_id = extract_rfq_id_from_interaction(
                &eth::Interaction {
                    target: liquorice_settlement_address,
                    call_data: Bytes::new(),
                    value: 0.into(),
                },
                liquorice_settlement_address,
            );

            assert!(rfq_id.is_none());
        }

        #[test]
        fn test_returns_none_for_different_target() {
            let rfq_id = extract_rfq_id_from_interaction(
                &eth::Interaction {
                    target: Address::random(),
                    call_data: Bytes::new(),
                    value: 0.into(),
                },
                Address::random(),
            );

            assert!(rfq_id.is_none());
        }
    }
}
