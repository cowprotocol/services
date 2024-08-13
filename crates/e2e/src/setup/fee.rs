pub struct ProtocolFeesConfig(pub Vec<ProtocolFee>);

#[derive(Clone)]
pub struct ProtocolFee {
    pub policy: FeePolicyKind,
    pub policy_order_class: FeePolicyOrderClass,
}

#[derive(Clone)]
pub enum FeePolicyOrderClass {
    Market,
    Limit,
    Any,
}

impl std::fmt::Display for FeePolicyOrderClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeePolicyOrderClass::Market => write!(f, "market"),
            FeePolicyOrderClass::Limit => write!(f, "limit"),
            FeePolicyOrderClass::Any => write!(f, "any"),
        }
    }
}

#[derive(Clone)]
pub enum FeePolicyKind {
    /// How much of the order's surplus should be taken as a protocol fee.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// How much of the order's volume should be taken as a protocol fee.
    Volume { factor: f64 },
    /// How much of the order's price improvement should be taken as a protocol
    /// fee where price improvement is a difference between the executed price
    /// and the best quote.
    PriceImprovement { factor: f64, max_volume_factor: f64 },
}

impl std::fmt::Display for ProtocolFee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let order_class_str = &self.policy_order_class.to_string();
        match &self.policy {
            FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => write!(
                f,
                "surplus:{}:{}:{}",
                factor, max_volume_factor, order_class_str
            ),
            FeePolicyKind::Volume { factor } => {
                write!(f, "volume:{}:{}", factor, order_class_str)
            }
            FeePolicyKind::PriceImprovement {
                factor,
                max_volume_factor,
            } => write!(
                f,
                "priceImprovement:{}:{}:{}",
                factor, max_volume_factor, order_class_str
            ),
        }
    }
}

impl std::fmt::Display for ProtocolFeesConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fees_str = self
            .0
            .iter()
            .map(|fee| fee.to_string())
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "--fee-policies={}", fees_str)
    }
}
