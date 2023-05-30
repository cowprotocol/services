use {
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, Zero},
    chrono::Utc,
    database::{
        onchain_broadcasted_orders::OnchainOrderPlacementError as DbOnchainOrderPlacementError,
        orders::{
            BuyTokenDestination as DbBuyTokenDestination,
            ExecutionTime,
            FullOrder as FullOrderDb,
            OrderClass as DbOrderClass,
            OrderKind as DbOrderKind,
            SellTokenSource as DbSellTokenSource,
            SigningScheme as DbSigningScheme,
        },
    },
    ethcontract::{H160, H256},
    model::{
        app_id::AppDataHash,
        interaction::InteractionData,
        order::{
            BuyTokenDestination,
            EthflowData,
            Interactions,
            LimitOrderClass,
            OnchainOrderData,
            OnchainOrderPlacementError,
            Order,
            OrderClass,
            OrderData,
            OrderKind,
            OrderMetadata,
            OrderStatus,
            OrderUid,
            SellTokenSource,
        },
        signature::{Signature, SigningScheme},
    },
    number_conversions::{big_decimal_to_big_uint, big_decimal_to_u256},
};

pub fn full_order_into_model_order(order: FullOrderDb) -> Result<Order> {
    let status = calculate_status(&order);
    let pre_interactions = extract_interactions(&order, ExecutionTime::Pre)?;
    let post_interactions = extract_interactions(&order, ExecutionTime::Post)?;
    let ethflow_data = if let Some((refund_tx, user_valid_to)) = order.ethflow_data {
        Some(EthflowData {
            user_valid_to,
            refund_tx_hash: refund_tx.map(|hash| H256(hash.0)),
        })
    } else {
        None
    };
    let onchain_user = order.onchain_user.map(|onchain_user| H160(onchain_user.0));
    let class = order_class_from(&order);
    let onchain_placement_error = onchain_order_placement_error_from(&order);
    let onchain_order_data = onchain_user.map(|onchain_user| OnchainOrderData {
        sender: onchain_user,
        placement_error: onchain_placement_error,
    });
    let full_fee_amount =
        big_decimal_to_u256(&order.full_fee_amount).context("full_fee_amount is not U256")?;
    let fee_amount = big_decimal_to_u256(&order.fee_amount).context("fee_amount is not U256")?;
    let solver_fee = match &class {
        // Liquidity orders should never have a fee unless the owner bribes the protocol by setting
        // one themselves.
        OrderClass::Liquidity => fee_amount,
        // We can't use `surplus_fee` or `fee_amount` here because those values include subsidies.
        // All else being equal a solver would then prefer including an unsubsidized order over a
        // subsidized one which we don't want.
        OrderClass::Limit(_) | OrderClass::Market => full_fee_amount,
    };

    let metadata = OrderMetadata {
        creation_date: order.creation_timestamp,
        owner: H160(order.owner.0),
        uid: OrderUid(order.uid.0),
        available_balance: Default::default(),
        executed_buy_amount: big_decimal_to_big_uint(&order.sum_buy)
            .context("executed buy amount is not an unsigned integer")?,
        executed_sell_amount: big_decimal_to_big_uint(&order.sum_sell)
            .context("executed sell amount is not an unsigned integer")?,
        // Executed fee amounts and sell amounts before fees are capped by
        // order's fee and sell amounts, and thus can always fit in a `U256`
        // - as it is limited by the order format.
        executed_sell_amount_before_fees: big_decimal_to_u256(&(order.sum_sell - &order.sum_fee))
            .context(
            "executed sell amount before fees does not fit in a u256",
        )?,
        executed_fee_amount: big_decimal_to_u256(&order.sum_fee)
            .context("executed fee amount is not a valid u256")?,
        invalidated: order.invalidated,
        status,
        is_liquidity_order: class == OrderClass::Liquidity,
        class,
        settlement_contract: H160(order.settlement_contract.0),
        full_fee_amount,
        solver_fee,
        ethflow_data,
        onchain_user,
        onchain_order_data,
    };
    let data = OrderData {
        sell_token: H160(order.sell_token.0),
        buy_token: H160(order.buy_token.0),
        receiver: order.receiver.map(|address| H160(address.0)),
        sell_amount: big_decimal_to_u256(&order.sell_amount).context("sell_amount is not U256")?,
        buy_amount: big_decimal_to_u256(&order.buy_amount).context("buy_amount is not U256")?,
        valid_to: order.valid_to.try_into().context("valid_to is not u32")?,
        app_data: AppDataHash(order.app_data.0),
        fee_amount,
        kind: order_kind_from(order.kind),
        partially_fillable: order.partially_fillable,
        sell_token_balance: sell_token_source_from(order.sell_token_balance),
        buy_token_balance: buy_token_destination_from(order.buy_token_balance),
    };
    let signing_scheme = signing_scheme_from(order.signing_scheme);
    let signature = Signature::from_bytes(signing_scheme, &order.signature)?;
    Ok(Order {
        metadata,
        data,
        signature,
        interactions: Interactions {
            pre: pre_interactions,
            post: post_interactions,
        },
    })
}

pub fn extract_interactions(
    order: &FullOrderDb,
    execution: ExecutionTime,
) -> Result<Vec<InteractionData>> {
    let interactions = match execution {
        ExecutionTime::Pre => &order.pre_interactions,
        ExecutionTime::Post => &order.post_interactions,
    };
    interactions
        .iter()
        .map(|interaction| {
            Ok(InteractionData {
                target: H160(interaction.0 .0),
                value: big_decimal_to_u256(&interaction.1)
                    .context("interaction value is not U256")?,
                call_data: interaction.2.to_vec(),
            })
        })
        .collect()
}

pub fn order_kind_into(kind: OrderKind) -> DbOrderKind {
    match kind {
        OrderKind::Buy => DbOrderKind::Buy,
        OrderKind::Sell => DbOrderKind::Sell,
    }
}

pub fn order_kind_from(kind: DbOrderKind) -> OrderKind {
    match kind {
        DbOrderKind::Buy => OrderKind::Buy,
        DbOrderKind::Sell => OrderKind::Sell,
    }
}

pub fn order_class_into(class: &OrderClass) -> DbOrderClass {
    match class {
        OrderClass::Market => DbOrderClass::Market,
        OrderClass::Liquidity => DbOrderClass::Liquidity,
        OrderClass::Limit(_) => DbOrderClass::Limit,
    }
}

pub fn onchain_order_placement_error_from(
    order: &FullOrderDb,
) -> Option<OnchainOrderPlacementError> {
    match order.onchain_placement_error {
        Some(DbOnchainOrderPlacementError::InvalidOrderData) => {
            Some(OnchainOrderPlacementError::InvalidOrderData)
        }
        Some(DbOnchainOrderPlacementError::QuoteNotFound) => {
            Some(OnchainOrderPlacementError::QuoteNotFound)
        }
        Some(DbOnchainOrderPlacementError::PreValidationError) => {
            Some(OnchainOrderPlacementError::PreValidationError)
        }
        Some(DbOnchainOrderPlacementError::DisabledOrderClass) => {
            Some(OnchainOrderPlacementError::DisabledOrderClass)
        }
        Some(DbOnchainOrderPlacementError::ValidToTooFarInFuture) => {
            Some(OnchainOrderPlacementError::ValidToTooFarInTheFuture)
        }
        Some(DbOnchainOrderPlacementError::InvalidQuote) => {
            Some(OnchainOrderPlacementError::InvalidQuote)
        }
        Some(DbOnchainOrderPlacementError::InsufficientFee) => {
            Some(OnchainOrderPlacementError::InsufficientFee)
        }
        Some(DbOnchainOrderPlacementError::Other) => Some(OnchainOrderPlacementError::Other),
        None => None,
    }
}

pub fn order_class_from(order: &FullOrderDb) -> OrderClass {
    match order.class {
        DbOrderClass::Market => OrderClass::Market,
        DbOrderClass::Liquidity => OrderClass::Liquidity,
        DbOrderClass::Limit => OrderClass::Limit(LimitOrderClass {
            surplus_fee: order
                .surplus_fee
                .as_ref()
                .map(|fee| big_decimal_to_u256(fee).unwrap()),
            surplus_fee_timestamp: order.surplus_fee_timestamp,
            executed_surplus_fee: order
                .executed_surplus_fee
                .as_ref()
                .and_then(big_decimal_to_u256),
        }),
    }
}

pub fn sell_token_source_into(source: SellTokenSource) -> DbSellTokenSource {
    match source {
        SellTokenSource::Erc20 => DbSellTokenSource::Erc20,
        SellTokenSource::Internal => DbSellTokenSource::Internal,
        SellTokenSource::External => DbSellTokenSource::External,
    }
}

pub fn sell_token_source_from(source: DbSellTokenSource) -> SellTokenSource {
    match source {
        DbSellTokenSource::Erc20 => SellTokenSource::Erc20,
        DbSellTokenSource::Internal => SellTokenSource::Internal,
        DbSellTokenSource::External => SellTokenSource::External,
    }
}

pub fn buy_token_destination_into(destination: BuyTokenDestination) -> DbBuyTokenDestination {
    match destination {
        BuyTokenDestination::Erc20 => DbBuyTokenDestination::Erc20,
        BuyTokenDestination::Internal => DbBuyTokenDestination::Internal,
    }
}

pub fn buy_token_destination_from(destination: DbBuyTokenDestination) -> BuyTokenDestination {
    match destination {
        DbBuyTokenDestination::Erc20 => BuyTokenDestination::Erc20,
        DbBuyTokenDestination::Internal => BuyTokenDestination::Internal,
    }
}

pub fn signing_scheme_into(scheme: SigningScheme) -> DbSigningScheme {
    match scheme {
        SigningScheme::Eip712 => DbSigningScheme::Eip712,
        SigningScheme::EthSign => DbSigningScheme::EthSign,
        SigningScheme::Eip1271 => DbSigningScheme::Eip1271,
        SigningScheme::PreSign => DbSigningScheme::PreSign,
    }
}

pub fn signing_scheme_from(scheme: DbSigningScheme) -> SigningScheme {
    match scheme {
        DbSigningScheme::Eip712 => SigningScheme::Eip712,
        DbSigningScheme::EthSign => SigningScheme::EthSign,
        DbSigningScheme::Eip1271 => SigningScheme::Eip1271,
        DbSigningScheme::PreSign => SigningScheme::PreSign,
    }
}

fn calculate_status(order: &FullOrderDb) -> OrderStatus {
    match order.kind {
        DbOrderKind::Buy => {
            if is_buy_order_filled(&order.buy_amount, &order.sum_buy) {
                return OrderStatus::Fulfilled;
            }
        }
        DbOrderKind::Sell => {
            if is_sell_order_filled(&order.sell_amount, &order.sum_sell, &order.sum_fee) {
                return OrderStatus::Fulfilled;
            }
        }
    }
    if order.invalidated {
        return OrderStatus::Cancelled;
    }
    if order.valid_to() < Utc::now().timestamp() {
        return OrderStatus::Expired;
    }
    if order.presignature_pending {
        return OrderStatus::PresignaturePending;
    }
    OrderStatus::Open
}

fn is_sell_order_filled(
    amount: &BigDecimal,
    executed_amount: &BigDecimal,
    executed_fee: &BigDecimal,
) -> bool {
    if executed_amount.is_zero() {
        return false;
    }
    let total_amount = executed_amount - executed_fee;
    total_amount == *amount
}

fn is_buy_order_filled(amount: &BigDecimal, executed_amount: &BigDecimal) -> bool {
    !executed_amount.is_zero() && *amount == *executed_amount
}

#[cfg(test)]
mod tests {
    use {super::*, chrono::Duration, database::byte_array::ByteArray};

    #[test]
    fn order_status() {
        let valid_to_timestamp = Utc::now() + Duration::days(1);

        let order_row = || FullOrderDb {
            uid: ByteArray([0; 56]),
            owner: ByteArray([0; 20]),
            creation_timestamp: Utc::now(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: BigDecimal::from(1),
            buy_amount: BigDecimal::from(1),
            valid_to: valid_to_timestamp.timestamp(),
            app_data: ByteArray([0; 32]),
            fee_amount: BigDecimal::default(),
            full_fee_amount: BigDecimal::default(),
            kind: DbOrderKind::Sell,
            class: DbOrderClass::Liquidity,
            partially_fillable: true,
            signature: vec![0; 65],
            receiver: None,
            sum_sell: BigDecimal::default(),
            sum_buy: BigDecimal::default(),
            sum_fee: BigDecimal::default(),
            invalidated: false,
            signing_scheme: DbSigningScheme::Eip712,
            settlement_contract: ByteArray([0; 20]),
            sell_token_balance: DbSellTokenSource::External,
            buy_token_balance: DbBuyTokenDestination::Internal,
            presignature_pending: false,
            pre_interactions: Vec::new(),
            post_interactions: Vec::new(),
            ethflow_data: None,
            onchain_user: None,
            onchain_placement_error: None,
            surplus_fee: Default::default(),
            surplus_fee_timestamp: Default::default(),
            executed_surplus_fee: Default::default(),
            executed_solver_fee: Default::default(),
        };

        // Open - sell (filled - 0%)
        assert_eq!(calculate_status(&order_row()), OrderStatus::Open);

        // Open - sell (almost filled - 99.99%)
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(10_000),
                sum_sell: BigDecimal::from(9_999),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Open - with presignature
        assert_eq!(
            calculate_status(&FullOrderDb {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: false,
                ..order_row()
            }),
            OrderStatus::Open
        );

        // PresignaturePending - without presignature
        assert_eq!(
            calculate_status(&FullOrderDb {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: true,
                ..order_row()
            }),
            OrderStatus::PresignaturePending
        );

        // Filled - sell (filled - 100%)
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(3),
                sum_fee: BigDecimal::from(1),
                ..order_row()
            }),
            OrderStatus::Fulfilled
        );

        // Open - buy (filled - 0%)
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(0),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Open - buy (almost filled - 99.99%)
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(10_000),
                sum_buy: BigDecimal::from(9_999),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Filled - buy (filled - 100%)
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(1),
                ..order_row()
            }),
            OrderStatus::Fulfilled
        );

        // Cancelled - no fills - sell
        assert_eq!(
            calculate_status(&FullOrderDb {
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - sell
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(1),
                sum_fee: BigDecimal::default(),
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - buy
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(2),
                sum_buy: BigDecimal::from(1),
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Expired - no fills
        let valid_to_yesterday = Utc::now() - Duration::days(1);

        assert_eq!(
            calculate_status(&FullOrderDb {
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - partial fill - sell
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(1),
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - partial fill - buy
        assert_eq!(
            calculate_status(&FullOrderDb {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(2),
                sum_buy: BigDecimal::from(1),
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - with pending presignature
        assert_eq!(
            calculate_status(&FullOrderDb {
                signing_scheme: DbSigningScheme::PreSign,
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                presignature_pending: true,
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - for ethflow orders
        assert_eq!(
            calculate_status(&FullOrderDb {
                invalidated: false,
                ethflow_data: Some((None, valid_to_yesterday.timestamp())),
                ..order_row()
            }),
            OrderStatus::Expired
        );
    }
}
