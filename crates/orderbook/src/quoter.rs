use {
    crate::app_data,
    chrono::{TimeZone, Utc},
    model::{
        order::OrderCreationAppData,
        quote::{OrderQuote, OrderQuoteRequest, OrderQuoteResponse, PriceQuality},
    },
    shared::{
        order_quoting::{CalculateQuoteError, OrderQuoting, QuoteParameters},
        order_validation::{
            AppDataValidationError,
            OrderValidating,
            PartialValidationError,
            PreOrderData,
        },
        price_estimation::Verification,
        trade_finding,
    },
    std::sync::Arc,
    thiserror::Error,
};

/// A high-level interface for handling API quote requests.
pub struct QuoteHandler {
    order_validator: Arc<dyn OrderValidating>,
    optimal_quoter: Arc<dyn OrderQuoting>,
    fast_quoter: Arc<dyn OrderQuoting>,
    app_data: Arc<app_data::Registry>,
}

impl QuoteHandler {
    pub fn new(
        order_validator: Arc<dyn OrderValidating>,
        quoter: Arc<dyn OrderQuoting>,
        app_data: Arc<app_data::Registry>,
    ) -> Self {
        Self {
            order_validator,
            optimal_quoter: quoter.clone(),
            fast_quoter: quoter,
            app_data,
        }
    }

    pub fn with_fast_quoter(mut self, fast_quoter: Arc<dyn OrderQuoting>) -> Self {
        self.fast_quoter = fast_quoter;
        self
    }
}

impl QuoteHandler {
    pub async fn calculate_quote(
        &self,
        request: &OrderQuoteRequest,
    ) -> Result<OrderQuoteResponse, OrderQuoteError> {
        tracing::debug!(?request, "calculating quote");

        let full_app_data_override = match request.app_data {
            OrderCreationAppData::Hash { hash } => self.app_data.find(&hash).await.unwrap_or(None),
            _ => None,
        };

        let app_data = self
            .order_validator
            .validate_app_data(&request.app_data, &full_app_data_override)?;

        let order = PreOrderData::from(request);
        let valid_to = order.valid_to;
        self.order_validator.partial_validate(order).await?;

        let params = QuoteParameters {
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            side: request.side,
            verification: Verification {
                from: request.from,
                receiver: request.receiver.unwrap_or(request.from),
                sell_token_source: request.sell_token_balance,
                buy_token_destination: request.buy_token_balance,
                pre_interactions: trade_finding::map_interactions(&app_data.interactions.pre),
                post_interactions: trade_finding::map_interactions(&app_data.interactions.post),
            },
            signing_scheme: request.signing_scheme,
            additional_gas: app_data.inner.protocol.hooks.gas_limit(),
        };

        let quote = match request.price_quality {
            PriceQuality::Optimal | PriceQuality::Verified => {
                let quote = self.optimal_quoter.calculate_quote(params).await?;
                self.optimal_quoter
                    .store_quote(quote)
                    .await
                    .map_err(CalculateQuoteError::Other)?
            }
            PriceQuality::Fast => {
                let mut quote = self.fast_quoter.calculate_quote(params).await?;
                // We maintain an API guarantee that fast quotes always have an expiry of zero,
                // because they're not very accurate and can be considered to
                // expire immediately.
                quote.data.expiration = Utc.timestamp_millis_opt(0).unwrap();
                quote
            }
        };

        let response = OrderQuoteResponse {
            quote: OrderQuote {
                sell_token: request.sell_token,
                buy_token: request.buy_token,
                receiver: request.receiver,
                sell_amount: quote.sell_amount,
                buy_amount: quote.buy_amount,
                valid_to,
                app_data: match &request.app_data {
                    OrderCreationAppData::Full { full } => OrderCreationAppData::Both {
                        full: full.clone(),
                        expected: request.app_data.hash(),
                    },
                    app_data => app_data.clone(),
                },
                fee_amount: quote.fee_amount,
                kind: quote.data.kind,
                partially_fillable: false,
                sell_token_balance: request.sell_token_balance,
                buy_token_balance: request.buy_token_balance,
                signing_scheme: request.signing_scheme.into(),
            },
            from: request.from,
            expiration: quote.data.expiration,
            id: quote.id,
            verified: quote.data.verified,
        };

        tracing::debug!(?response, "finished computing quote");
        Ok(response)
    }
}

/// Result from handling a quote request.
#[derive(Debug, Error)]
pub enum OrderQuoteError {
    #[error("error validating app data: {0:?}")]
    AppData(AppDataValidationError),

    #[error("error validating order data: {0:?}")]
    Order(PartialValidationError),

    #[error("error calculating quote: {0}")]
    CalculateQuote(#[from] CalculateQuoteError),
}

impl From<AppDataValidationError> for OrderQuoteError {
    fn from(err: AppDataValidationError) -> Self {
        Self::AppData(err)
    }
}

impl From<PartialValidationError> for OrderQuoteError {
    fn from(err: PartialValidationError) -> Self {
        Self::Order(err)
    }
}
