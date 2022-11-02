use crate::database::Postgres;
use anyhow::Result;
use chrono::Duration;
use model::{
    order::OrderKind,
    quote::{default_verification_gas_limit, OrderQuoteSide, QuoteSigningScheme, SellAmount},
    signature::Signature,
};
use shared::order_quoting::{OrderQuoting, QuoteParameters};
use std::sync::Arc;

/// Background task which quotes all limit orders and sets the surplus_fee for each one
/// to the fee returned by the quoting process. If quoting fails, the corresponding
/// order is skipped.
pub struct LimitOrderQuoter {
    pub limit_order_age: Duration,
    pub loop_delay: std::time::Duration,
    pub quoter: Arc<dyn OrderQuoting>,
    pub database: Postgres,
}

impl LimitOrderQuoter {
    pub fn spawn(self) {
        tokio::spawn(async move {
            loop {
                if let Err(err) = self.update().await {
                    tracing::error!(?err, "failed to update limit order surplus");
                }
                tokio::time::sleep(self.loop_delay).await;
            }
        });
    }

    async fn update(&self) -> Result<()> {
        let orders = self
            .database
            .limit_orders_with_outdated_fees(self.limit_order_age)
            .await?;
        for order in orders {
            match self
                .quoter
                .calculate_quote(QuoteParameters {
                    sell_token: order.data.sell_token,
                    buy_token: order.data.buy_token,
                    side: match order.data.kind {
                        OrderKind::Buy => OrderQuoteSide::Buy {
                            buy_amount_after_fee: order.data.buy_amount,
                        },
                        OrderKind::Sell => OrderQuoteSide::Sell {
                            sell_amount: SellAmount::BeforeFee {
                                value: order.data.sell_amount + order.data.fee_amount,
                            },
                        },
                    },
                    from: order.metadata.owner,
                    app_data: order.data.app_data,
                    signing_scheme: match order.signature {
                        Signature::Eip712(_) => QuoteSigningScheme::Eip712,
                        Signature::EthSign(_) => QuoteSigningScheme::EthSign,
                        Signature::Eip1271(_) => QuoteSigningScheme::Eip1271 {
                            onchain_order: false,
                            verification_gas_limit: default_verification_gas_limit(),
                        },
                        Signature::PreSign => QuoteSigningScheme::PreSign {
                            onchain_order: false,
                        },
                    },
                })
                .await
            {
                Ok(quote) => {
                    if let Err(err) = self
                        .database
                        .update_surplus_fee(&order.metadata.uid, quote.fee_amount)
                        .await
                    {
                        tracing::error!(
                            ?err,
                            ?quote,
                            "failed to update quote surplus fee, skipping"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        order_uid =% order.metadata.uid, ?err,
                        "skipped limit order due to quoting error"
                    );
                }
            }
        }
        Ok(())
    }
}
