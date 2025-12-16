use {
    alloy::primitives::Address,
    anyhow,
    model::{
        order::{
            AppdataFromMismatch, BuyTokenDestination, OrderClass, OrderCreationAppData,
            SellTokenSource, OrderData,
        },
        signature::{self, SigningScheme},
    },
    app_data::{AppDataHash, ValidatedAppData},
    price_estimation::PriceEstimationError,
    order_quoting::CalculateQuoteError,
    model::interaction::InteractionData,
    ethcontract::H256,
    std::time::Duration,
    time,
};

/// Pre-order data for validation
///
/// Contains the essential order properties needed for validation.
/// This is extracted early and used by all validation strategies.
#[derive(Debug, Clone)]
pub struct PreOrderData {
    pub owner: Address,
    pub sell_token: Address,
    pub buy_token: Address,
    pub receiver: Address,
    pub valid_to: u32,
    pub partially_fillable: bool,
    pub buy_token_balance: BuyTokenDestination,
    pub sell_token_balance: SellTokenSource,
    pub signing_scheme: SigningScheme,
    pub class: OrderClass,
}

impl PreOrderData {
    /// Create PreOrderData from order creation data
    pub fn from_order_creation(
        owner: Address,
        order: &OrderData,
        signing_scheme: SigningScheme,
    ) -> Self {
        let receiver = actual_receiver(owner, order);

        Self {
            owner,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            receiver,
            valid_to: order.valid_to,
            partially_fillable: order.partially_fillable,
            buy_token_balance: order.buy_token_balance,
            sell_token_balance: order.sell_token_balance,
            signing_scheme,
            class: if order.fee_amount.is_zero() {
                OrderClass::Limit
            } else {
                OrderClass::Market
            },
        }
    }
}

/// Validate app data and interactions
#[derive(Debug, Clone)]
pub struct OrderAppData {
    pub inner: ValidatedAppData,
    pub interactions: model::order::Interactions,
}

// ============================================================================
// Error Types
// ============================================================================

/// Partial validation errors (intrinsic phase)
#[derive(Debug)]
pub enum PartialValidationError {
    Forbidden,
    ValidTo(OrderValidToError),
    InvalidNativeSellToken,
    SameBuyAndSellToken,
    UnsupportedBuyTokenDestination(BuyTokenDestination),
    UnsupportedSellTokenSource(SellTokenSource),
    UnsupportedOrderType,
    UnsupportedToken { token: Address, reason: String },
    Other(anyhow::Error),
}

impl PartialValidationError {
    /// Convert to ValidationError
    pub fn into_validation_error(self) -> ValidationError {
        ValidationError::Partial(self)
    }
}

impl From<OrderValidToError> for PartialValidationError {
    fn from(err: OrderValidToError) -> Self {
        Self::ValidTo(err)
    }
}

/// App data validation errors
#[derive(Debug)]
pub enum AppDataValidationError {
    Mismatch {
        provided: AppDataHash,
        actual: AppDataHash,
    },
    Invalid(anyhow::Error),
}

impl AppDataValidationError {
    /// Convert to ValidationError
    pub fn into_validation_error(self) -> ValidationError {
        ValidationError::AppData(self)
    }
}

/// Order validity period errors
#[derive(Debug)]
pub enum OrderValidToError {
    Insufficient,
    Excessive,
}

/// Comprehensive validation error type
#[derive(Debug)]
pub enum ValidationError {
    // Intrinsic errors (from partial validation)
    Partial(PartialValidationError),
    AppData(AppDataValidationError),

    // Signature errors
    InvalidSignature,
    WrongOwner(signature::Recovered),
    InvalidEip1271Signature(H256),
    MissingFrom,
    AppdataFromMismatch(AppdataFromMismatch),
    IncompatibleSigningScheme,

    // Amount errors
    ZeroAmount,
    SellAmountOverflow,
    NonZeroFee,

    // Balance/allowance errors
    InsufficientBalance,
    InsufficientAllowance,
    TransferSimulationFailed,

    // Quote and price errors
    PriceForQuote(PriceEstimationError),
    QuoteNotVerified,

    // Limit order errors
    TooManyLimitOrders,

    // Gas errors
    TooMuchGas,

    // Wrapper errors
    WrapperValidationFailed {
        wrapper: Address,
        reason: String,
    },

    // Catch-all
    Other(anyhow::Error),
}

impl From<AppDataValidationError> for ValidationError {
    fn from(value: AppDataValidationError) -> Self {
        Self::AppData(value)
    }
}

impl From<signature::VerificationError> for ValidationError {
    fn from(err: signature::VerificationError) -> Self {
        match err {
            signature::VerificationError::UnableToRecoverSigner(_) => Self::InvalidSignature,
            signature::VerificationError::UnexpectedSigner(recovered) => {
                Self::WrongOwner(recovered)
            }
            signature::VerificationError::MissingFrom => Self::MissingFrom,
            signature::VerificationError::AppdataFromMismatch(mismatch) => {
                Self::AppdataFromMismatch(mismatch)
            }
        }
    }
}

impl From<CalculateQuoteError> for ValidationError {
    fn from(err: CalculateQuoteError) -> Self {
        match err {
            CalculateQuoteError::Price {
                source: PriceEstimationError::UnsupportedToken { token, reason },
                ..
            } => ValidationError::Partial(PartialValidationError::UnsupportedToken {
                token,
                reason,
            }),
            CalculateQuoteError::Price {
                source: PriceEstimationError::ProtocolInternal(err),
                ..
            }
            | CalculateQuoteError::Other(err) => ValidationError::Other(err),
            CalculateQuoteError::Price { source, .. } => ValidationError::PriceForQuote(source),
            CalculateQuoteError::QuoteNotVerified => ValidationError::QuoteNotVerified,
            // This should never happen
            CalculateQuoteError::SellAmountOverflow => ValidationError::SellAmountOverflow,
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::Partial(e) => write!(f, "partial validation error: {:?}", e),
            ValidationError::AppData(e) => write!(f, "app data error: {:?}", e),
            ValidationError::InvalidSignature => write!(f, "invalid signature"),
            ValidationError::WrongOwner(_) => write!(f, "wrong owner"),
            ValidationError::InvalidEip1271Signature(_) => write!(f, "invalid EIP-1271 signature"),
            ValidationError::MissingFrom => write!(f, "missing from"),
            ValidationError::AppdataFromMismatch(_) => write!(f, "appdata from mismatch"),
            ValidationError::IncompatibleSigningScheme => write!(f, "incompatible signing scheme"),
            ValidationError::ZeroAmount => write!(f, "zero amount"),
            ValidationError::SellAmountOverflow => write!(f, "sell amount overflow"),
            ValidationError::NonZeroFee => write!(f, "non-zero fee"),
            ValidationError::InsufficientBalance => write!(f, "insufficient balance"),
            ValidationError::InsufficientAllowance => write!(f, "insufficient allowance"),
            ValidationError::TransferSimulationFailed => write!(f, "transfer simulation failed"),
            ValidationError::PriceForQuote(_) => write!(f, "price estimation error"),
            ValidationError::QuoteNotVerified => write!(f, "quote not verified"),
            ValidationError::TooManyLimitOrders => write!(f, "too many limit orders"),
            ValidationError::TooMuchGas => write!(f, "too much gas"),
            ValidationError::WrapperValidationFailed { wrapper, reason } => {
                write!(f, "wrapper validation failed for {}: {}", wrapper, reason)
            }
            ValidationError::Other(err) => write!(f, "validation error: {}", err),
        }
    }
}

impl std::error::Error for ValidationError {}

// ============================================================================
// Helper Functions
// ============================================================================

/// Order validity period configuration
///
/// Defines the allowed range for order validity periods based on order class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderValidPeriodConfiguration {
    /// Minimum validity duration required for all orders
    pub min: Duration,

    /// Maximum validity duration for market orders
    pub max_market: Duration,

    /// Maximum validity duration for limit orders
    pub max_limit: Duration,
}

impl OrderValidPeriodConfiguration {
    /// Creates a configuration where any `validTo` is accepted.
    pub fn any() -> Self {
        Self {
            min: Duration::ZERO,
            max_market: Duration::MAX,
            max_limit: Duration::MAX,
        }
    }

    /// Validates an order's timestamp based on additional data.
    pub fn validate_period(&self, order: &PreOrderData) -> Result<(), OrderValidToError> {
        let now = time::now_in_epoch_seconds();
        if order.valid_to < time::timestamp_after_duration(now, self.min) {
            return Err(OrderValidToError::Insufficient);
        }
        if order.valid_to > time::timestamp_after_duration(now, self.max(order)) {
            return Err(OrderValidToError::Excessive);
        }

        Ok(())
    }

    /// Returns the maximum valid timestamp for the specified order.
    fn max(&self, order: &PreOrderData) -> Duration {
        // For now, there is no maximum `validTo` for pre-sign orders as a hack
        // for dealing with signature collection times. We should probably
        // revisit this.
        if order.signing_scheme == SigningScheme::PreSign {
            return Duration::MAX;
        }

        match order.class {
            OrderClass::Market => self.max_market,
            OrderClass::Limit => self.max_limit,
            OrderClass::Liquidity => Duration::MAX,
        }
    }
}

/// Get the actual receiver address (use owner if receiver is zero)
pub(crate) fn actual_receiver(owner: Address, order: &OrderData) -> Address {
    let receiver = order.receiver.unwrap_or_default();
    if receiver.is_zero() { owner } else { receiver }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_display() {
        let err = ValidationError::ZeroAmount;
        let display = format!("{}", err);
        assert!(display.contains("zero amount"));
    }
}
