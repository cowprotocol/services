#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, strum::EnumIter)]
pub enum OrderOperation {
    Created,
    Cancelled,
}

pub fn operation_label(op: &OrderOperation) -> &'static str {
    match op {
        OrderOperation::Created => "created",
        OrderOperation::Cancelled => "cancelled",
    }
}

pub fn order_class_label(class: &model::order::OrderClass) -> &'static str {
    match class {
        model::order::OrderClass::Market => {
            db_order_class_label(&database::orders::OrderClass::Market)
        }
        model::order::OrderClass::Liquidity => {
            db_order_class_label(&database::orders::OrderClass::Liquidity)
        }
        model::order::OrderClass::Limit(_) => {
            db_order_class_label(&database::orders::OrderClass::Limit)
        }
    }
}

pub fn db_order_class_label(class: &database::orders::OrderClass) -> &'static str {
    match class {
        database::orders::OrderClass::Market => "user",
        database::orders::OrderClass::Liquidity => "liquidity",
        database::orders::OrderClass::Limit => "limit",
    }
}
