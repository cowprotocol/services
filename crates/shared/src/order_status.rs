use {
    chrono::Utc,
    database::orders::{FullOrder, OrderKind},
    model::order::OrderStatus,
};

pub fn calculate_status(order: &FullOrder) -> OrderStatus {
    match order.kind {
        OrderKind::Buy => {
            if order.is_buy_order_filled() {
                return OrderStatus::Fulfilled;
            }
        }
        OrderKind::Sell => {
            if order.is_sell_order_filled() {
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

#[cfg(test)]
mod test {
    use {
        crate::order_status::calculate_status,
        bigdecimal::BigDecimal,
        chrono::{Duration, Utc},
        database::{
            byte_array::ByteArray,
            orders::{
                BuyTokenDestination,
                FullOrder,
                OrderClass,
                OrderKind,
                SellTokenSource,
                SigningScheme,
            },
        },
        model::order::OrderStatus,
    };

    #[test]
    fn order_status() {
        let valid_to_timestamp = Utc::now() + Duration::days(1);

        let order_row = || FullOrder {
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
            kind: OrderKind::Sell,
            class: OrderClass::Liquidity,
            partially_fillable: true,
            signature: vec![0; 65],
            receiver: None,
            sum_sell: BigDecimal::default(),
            sum_buy: BigDecimal::default(),
            sum_fee: BigDecimal::default(),
            invalidated: false,
            signing_scheme: SigningScheme::Eip712,
            settlement_contract: ByteArray([0; 20]),
            sell_token_balance: SellTokenSource::External,
            buy_token_balance: BuyTokenDestination::Internal,
            presignature_pending: false,
            pre_interactions: Vec::new(),
            post_interactions: Vec::new(),
            ethflow_data: None,
            onchain_user: None,
            onchain_placement_error: None,
            executed_fee: Default::default(),
            executed_fee_token: ByteArray([1; 20]), // TODO surplus token
            full_app_data: Default::default(),
        };

        // Open - sell (filled - 0%)
        assert_eq!(calculate_status(&order_row()), OrderStatus::Open);

        // Open - sell (almost filled - 99.99%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Sell,
                sell_amount: BigDecimal::from(10_000),
                sum_sell: BigDecimal::from(9_999),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Open - with presignature
        assert_eq!(
            calculate_status(&FullOrder {
                signing_scheme: SigningScheme::PreSign,
                presignature_pending: false,
                ..order_row()
            }),
            OrderStatus::Open
        );

        // PresignaturePending - without presignature
        assert_eq!(
            calculate_status(&FullOrder {
                signing_scheme: SigningScheme::PreSign,
                presignature_pending: true,
                ..order_row()
            }),
            OrderStatus::PresignaturePending
        );

        // Filled - sell (filled - 100%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(3),
                sum_fee: BigDecimal::from(1),
                ..order_row()
            }),
            OrderStatus::Fulfilled
        );

        // Open - buy (filled - 0%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(0),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Open - buy (almost filled - 99.99%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Buy,
                buy_amount: BigDecimal::from(10_000),
                sum_buy: BigDecimal::from(9_999),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Filled - buy (filled - 100%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(1),
                ..order_row()
            }),
            OrderStatus::Fulfilled
        );

        // Cancelled - no fills - sell
        assert_eq!(
            calculate_status(&FullOrder {
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - sell
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Sell,
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
            calculate_status(&FullOrder {
                kind: OrderKind::Buy,
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
            calculate_status(&FullOrder {
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - partial fill - sell
        assert_eq!(
            calculate_status(&FullOrder {
                kind: OrderKind::Sell,
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
            calculate_status(&FullOrder {
                kind: OrderKind::Buy,
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
            calculate_status(&FullOrder {
                signing_scheme: SigningScheme::PreSign,
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                presignature_pending: true,
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - for ethflow orders
        assert_eq!(
            calculate_status(&FullOrder {
                invalidated: false,
                ethflow_data: Some((None, valid_to_yesterday.timestamp())),
                ..order_row()
            }),
            OrderStatus::Expired
        );
    }
}
