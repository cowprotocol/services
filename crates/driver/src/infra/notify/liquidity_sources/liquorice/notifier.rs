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
                liquorice::client::{BeforeSettleNotification, DefaultLiquoriceApi, NotifyQuery},
            },
        },
        util::Bytes,
    },
    anyhow::{Context, Result, anyhow},
    contracts::ILiquoriceSettlement,
    ethabi::Token,
    std::collections::HashSet,
};

pub struct Notifier {
    /// Liquorice API client
    liquorice_api: DefaultLiquoriceApi,
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
            liquorice_api: DefaultLiquoriceApi::new(
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
    fn extract_rfq_ids_from_settlement(&self, settlement: &Settlement) -> Result<HashSet<String>> {
        // Aggregate all interactions from the settlement
        let pre_interactions = settlement.pre_interactions();
        let interactions = settlement
            .interactions()
            .iter()
            .filter_map(|interaction| match interaction {
                solution::Interaction::Custom(custom) => Some(eth::Interaction {
                    target: custom.target.into(),
                    value: custom.value,
                    call_data: custom.call_data.clone(),
                }),
                solution::Interaction::Liquidity(_) => None,
            })
            .collect::<Vec<eth::Interaction>>();
        let post_interactions = settlement.post_interactions();

        // Extract RFQ IDs from the interactions
        let rfq_ids = pre_interactions
            .iter()
            .chain(interactions.iter())
            .chain(post_interactions.iter())
            .filter(|interaction| interaction.target == self.settlement_contract_address)
            .map(|interaction| Self::extract_rfq_id_from_calldata(&interaction.call_data))
            .collect::<Result<Vec<String>>>()?;

        Ok(HashSet::from_iter(rfq_ids.into_iter()))
    }

    /// Extracts rfqId from the `settleSingle` function call arguments of the
    /// ILiquoriceSettlement.sol interface <https://etherscan.io/address/0xaca684a3f64e0eae4812b734e3f8f205d3eed167#code#F6#L1>
    fn extract_rfq_id_from_calldata(calldata: &Bytes<Vec<u8>>) -> Result<String> {
        // Decode the calldata using the Liquorice settlement contract ABI
        let settle_single_function = ILiquoriceSettlement::raw_contract()
            .interface
            .abi
            .function("settleSingle")
            .unwrap();
        let tokens = settle_single_function
            .decode_output(calldata.0.as_slice())
            .context("Invalid output from settleSingle")?;

        // Token at index 1 is expected to be an instance of `Single` order
        // in the ILiquoriceSettlement.sol
        let rfq_id = tokens
            .get(1)
            .map(|token| match token {
                Token::Tuple(tokens) => {
                    // Token at index 0 is expected to be a string corresponding to `rfqId`
                    // field of the `Single` order
                    tokens
                        .first()
                        .map(|token| match token {
                            Token::String(rfq_id) => Ok(rfq_id.clone()),
                            _ => Err(anyhow!("Expected a string token for RFQ ID")),
                        })
                        .transpose()
                }
                _ => Err(anyhow!("Expected a tuple token for Liquorice single order")),
            })
            .transpose()?
            .flatten()
            .ok_or(anyhow!("RFQ ID not found in settlement calldata"));

        rfq_id
    }
}

#[async_trait::async_trait]
impl LiquiditySourcesNotifying for Notifier {
    async fn notify_before_settlement(&self, settlement: &Settlement) -> Result<()> {
        let rfq_ids = self.extract_rfq_ids_from_settlement(settlement)?;

        self.liquorice_api
            .notify(&NotifyQuery::BeforeSettle(BeforeSettleNotification {
                rfq_ids,
            }))
            .await
            .context("Failed to notify before_settle")
    }
}
