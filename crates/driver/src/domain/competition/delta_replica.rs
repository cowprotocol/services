use {
    crate::infra::observe::metrics,
    alloy::primitives::Address,
    eth_domain_types as eth,
    hex,
    serde::Deserialize,
    serde_json::Value,
    sha2::{Digest, Sha256},
    std::{collections::HashMap, str::FromStr},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported delta protocol version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid sequence range: from={from}, to={to}")]
    InvalidSequenceRange { from: u64, to: u64 },
    #[error("delta sequence mismatch: expected from={expected}, got from={got}")]
    SequenceMismatch { expected: u64, got: u64 },
    #[error("delta snapshot sequence mismatch: current={current}, snapshot={snapshot}")]
    SnapshotSequenceMismatch { current: u64, snapshot: u64 },
    #[error("order payload is missing uid")]
    MissingOrderUid,
    #[error("order uid has invalid format: {0}")]
    InvalidOrderUidFormat(String),
    #[error("order payload failed schema validation: {0}")]
    InvalidOrderSchema(String),
    #[error("price payload has invalid format: {0}")]
    InvalidPriceFormat(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: u32,
    #[serde(default)]
    pub auction_id: u64,
    pub sequence: u64,
    pub auction: RawAuctionData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAuctionData {
    pub orders: Vec<Value>,
    pub prices: HashMap<Address, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub version: u32,
    #[serde(default)]
    pub auction_id: u64,
    #[serde(default)]
    pub auction_sequence: u64,
    pub from_sequence: u64,
    pub to_sequence: u64,
    #[serde(default)]
    pub snapshot_sequence: Option<u64>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    AuctionChanged {
        new_auction_id: u64,
    },
    OrderAdded {
        order: Value,
    },
    OrderRemoved {
        uid: String,
    },
    OrderUpdated {
        order: Value,
    },
    PriceChanged {
        token: Address,
        price: Option<String>,
    },
    /// Unknown events are ignored for forward compatibility; breaking changes
    /// must bump the protocol version.
    #[serde(other)]
    Unknown,
}

/// Driver-side local replica of the order and price subset needed for delta
/// sync.
#[derive(Debug, Clone)]
pub struct Replica {
    sequence: u64,
    auction_id: u64,
    orders: HashMap<String, ReplicaOrder>,
    prices: HashMap<Address, String>,
    state: ReplicaState,
    last_update: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ReplicaChecksum {
    pub sequence: u64,
    pub order_uid_hash: String,
    pub price_hash: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicaState {
    Uninitialized,
    Syncing,
    Ready,
    Resyncing,
}

impl Default for Replica {
    fn default() -> Self {
        Self {
            sequence: 0,
            auction_id: 0,
            orders: HashMap::new(),
            prices: HashMap::new(),
            state: ReplicaState::Uninitialized,
            last_update: None,
        }
    }
}

impl Replica {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn auction_id(&self) -> u64 {
        self.auction_id
    }

    pub(crate) fn orders(&self) -> &HashMap<String, ReplicaOrder> {
        &self.orders
    }

    pub fn prices(&self) -> &HashMap<Address, String> {
        &self.prices
    }

    pub fn state(&self) -> ReplicaState {
        self.state
    }

    pub fn last_update(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_update
    }

    pub fn checksum(&self) -> ReplicaChecksum {
        let order_uid_hash = Self::checksum_order_uids(&self.orders);
        let price_hash = Self::checksum_prices(&self.prices);
        ReplicaChecksum {
            sequence: self.sequence,
            order_uid_hash,
            price_hash,
        }
    }

    pub fn set_state(&mut self, state: ReplicaState) {
        self.state = state;
    }

    pub fn apply_snapshot(&mut self, snapshot: Snapshot) -> Result<(), Error> {
        Self::ensure_version(snapshot.version)?;

        for (token, price) in &snapshot.auction.prices {
            validate_price_string(*token, price)?;
        }
        let new_orders = snapshot
            .auction
            .orders
            .into_iter()
            .map(|order| {
                let uid = order_uid(&order)?;
                let order = Self::parse_order(&order)?;
                Ok((uid, order))
            })
            .collect::<Result<HashMap<_, _>, Error>>()?;
        let new_prices = snapshot.auction.prices;
        let new_sequence = snapshot.sequence;

        self.sequence = new_sequence;
        self.auction_id = snapshot.auction_id;
        self.orders = new_orders;
        self.prices = new_prices;
        self.state = ReplicaState::Ready;
        self.last_update = Some(chrono::Utc::now());
        metrics::get()
            .delta_replica_order_count
            .set(self.orders.len() as i64);

        Ok(())
    }

    pub fn apply_delta(&mut self, envelope: Envelope) -> Result<(), Error> {
        Self::ensure_version(envelope.version)?;

        if let Some(snapshot_sequence) = envelope.snapshot_sequence {
            if self.sequence < snapshot_sequence {
                return Err(Error::SnapshotSequenceMismatch {
                    current: self.sequence,
                    snapshot: snapshot_sequence,
                });
            }
        }

        if envelope.from_sequence > envelope.to_sequence {
            return Err(Error::InvalidSequenceRange {
                from: envelope.from_sequence,
                to: envelope.to_sequence,
            });
        }

        // At-least-once delivery can replay already-applied envelopes.
        if envelope.to_sequence <= self.sequence {
            return Ok(());
        }

        if envelope.from_sequence != self.sequence {
            return Err(Error::SequenceMismatch {
                expected: self.sequence,
                got: envelope.from_sequence,
            });
        }

        enum Mutation {
            UpsertOrder {
                uid: String,
                order: ReplicaOrder,
            },
            RemoveOrder {
                uid: String,
            },
            PriceChanged {
                token: Address,
                price: Option<String>,
            },
        }

        let mut mutations = Vec::with_capacity(envelope.events.len());
        for event in envelope.events {
            match event {
                Event::AuctionChanged { .. } => {}
                Event::OrderAdded { order } | Event::OrderUpdated { order } => {
                    let uid = order_uid(&order)?;
                    let order = Self::parse_order(&order)?;
                    mutations.push(Mutation::UpsertOrder { uid, order });
                }
                Event::OrderRemoved { uid } => {
                    if !is_valid_order_uid(&uid) {
                        return Err(Error::InvalidOrderUidFormat(uid));
                    }
                    mutations.push(Mutation::RemoveOrder { uid });
                }
                Event::PriceChanged { token, price } => {
                    if let Some(ref value) = price {
                        validate_price_string(token, value)?;
                    }
                    mutations.push(Mutation::PriceChanged { token, price });
                }
                Event::Unknown => {}
            }
        }

        for mutation in mutations {
            match mutation {
                Mutation::UpsertOrder { uid, order } => {
                    self.orders.insert(uid, order);
                }
                Mutation::RemoveOrder { uid } => {
                    if self.orders.remove(&uid).is_none() {
                        metrics::get()
                            .delta_replica_unknown_order_removals_total
                            .inc();
                        tracing::warn!(order_uid = %uid, "delta replica removed unknown order");
                    }
                }
                Mutation::PriceChanged { token, price } => {
                    if let Some(price) = price {
                        self.prices.insert(token, price);
                    } else {
                        self.prices.remove(&token);
                    }
                }
            }
        }

        self.sequence = envelope.to_sequence;
        self.auction_id = envelope.auction_id;
        self.state = ReplicaState::Ready;
        self.last_update = Some(chrono::Utc::now());
        metrics::get()
            .delta_replica_order_count
            .set(self.orders.len() as i64);
        Ok(())
    }

    fn ensure_version(version: u32) -> Result<(), Error> {
        if version == 1 {
            Ok(())
        } else {
            Err(Error::UnsupportedVersion(version))
        }
    }

    fn checksum_order_uids(orders: &HashMap<String, ReplicaOrder>) -> String {
        let mut uids = orders.keys().cloned().collect::<Vec<_>>();
        uids.sort();

        let mut hasher = Sha256::new();
        for uid in uids {
            let bytes = uid.strip_prefix("0x").unwrap_or(&uid);
            match hex::decode(bytes) {
                Ok(decoded) => hasher.update(decoded),
                Err(err) => {
                    metrics::get()
                        .delta_replica_checksum_decode_errors_total
                        .inc();
                    tracing::warn!(%uid, ?err, "checksum: failed to hex-decode order uid");
                    return "error".to_string();
                }
            }
        }
        format!("0x{}", const_hex::encode(hasher.finalize()))
    }

    fn checksum_prices(prices: &HashMap<Address, String>) -> String {
        let mut entries = prices.iter().collect::<Vec<_>>();
        entries.sort_by(|(lhs, _), (rhs, _)| lhs.as_slice().cmp(rhs.as_slice()));

        let mut hasher = Sha256::new();
        for (token, price) in entries {
            hasher.update(token.as_slice());
            let canonical = eth::U256::from_str(price)
                .map(|value| value.to_string())
                .unwrap_or_else(|_| price.to_string());
            hasher.update(canonical.as_bytes());
        }
        format!("0x{}", const_hex::encode(hasher.finalize()))
    }

    fn parse_order(order: &Value) -> Result<ReplicaOrder, Error> {
        validate_order_minimal(order)?;
        log_unknown_order_fields(order);
        serde_json::from_value::<ReplicaOrder>(order.clone())
            .map_err(|err| Error::InvalidOrderSchema(err.to_string()))
    }
}

type ReplicaOrder = crate::infra::api::routes::solve::dto::solve_request::Order;

const ORDER_REQUIRED_FIELDS: [&str; 5] =
    ["uid", "sellToken", "buyToken", "sellAmount", "buyAmount"];

const ORDER_KNOWN_FIELDS: [&str; 22] = [
    "uid",
    "sellToken",
    "buyToken",
    "sellAmount",
    "buyAmount",
    "protocolFees",
    "created",
    "validTo",
    "kind",
    "receiver",
    "owner",
    "partiallyFillable",
    "executed",
    "preInteractions",
    "postInteractions",
    "sellTokenBalance",
    "buyTokenBalance",
    "class",
    "appData",
    "signingScheme",
    "signature",
    "quote",
];

fn validate_order_minimal(order: &Value) -> Result<(), Error> {
    let obj = order
        .as_object()
        .ok_or_else(|| Error::InvalidOrderSchema("order payload is not an object".to_string()))?;

    for field in ORDER_REQUIRED_FIELDS {
        if obj.get(field).is_none() {
            metrics::get()
                .delta_replica_missing_required_fields_total
                .inc();
            return Err(Error::InvalidOrderSchema(format!(
                "order payload missing required field {field}"
            )));
        }
    }

    Ok(())
}

fn log_unknown_order_fields(order: &Value) {
    let Some(obj) = order.as_object() else {
        return;
    };

    let mut unknown = Vec::new();
    for key in obj.keys() {
        if !ORDER_KNOWN_FIELDS.contains(&key.as_str()) {
            unknown.push(key.clone());
        }
    }

    if !unknown.is_empty() {
        metrics::get()
            .delta_replica_unknown_fields_total
            .inc_by(unknown.len() as u64);
        tracing::warn!(unknown_fields = ?unknown, "delta replica order contains unknown fields");
    }
}

fn order_uid(order: &Value) -> Result<String, Error> {
    let uid = order
        .get("uid")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or(Error::MissingOrderUid)?;
    if !is_valid_order_uid(&uid) {
        return Err(Error::InvalidOrderUidFormat(uid));
    }
    Ok(uid)
}

fn is_valid_order_uid(uid: &str) -> bool {
    uid.starts_with("0x")
        && uid.len() == 114
        && (uid.len() - 2).is_multiple_of(2)
        && uid
            .as_bytes()
            .iter()
            .skip(2)
            .all(|byte| byte.is_ascii_hexdigit())
}

fn validate_price_string(token: Address, value: &str) -> Result<(), Error> {
    if value.is_empty() {
        return Err(Error::InvalidPriceFormat(format!("{token:?}:{value}")));
    }
    eth::U256::from_str(value)
        .map_err(|_| Error::InvalidPriceFormat(format!("{token:?}:{value}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_order(uid: &str) -> Value {
        serde_json::json!({
            "uid": uid,
            "sellToken": "0x0000000000000000000000000000000000000001",
            "buyToken": "0x0000000000000000000000000000000000000002",
            "sellAmount": "1",
            "buyAmount": "1",
            "protocolFees": [],
            "created": 1,
            "validTo": 100,
            "kind": "sell",
            "receiver": null,
            "owner": "0x0000000000000000000000000000000000000003",
            "partiallyFillable": false,
            "executed": "0",
            "preInteractions": [],
            "postInteractions": [],
            "class": "market",
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "signingScheme": "eip712",
            "signature": "0x00",
            "quote": null
        })
    }

    fn snapshot(sequence: u64, orders: Vec<Value>) -> Snapshot {
        Snapshot {
            version: 1,
            auction_id: 0,
            sequence,
            auction: RawAuctionData {
                orders,
                prices: HashMap::new(),
            },
        }
    }

    #[test]
    fn checksum_prices_canonicalizes_decimal_strings() {
        let token = Address::repeat_byte(0xAA);
        let mut prices = HashMap::new();
        prices.insert(token, "0010".to_string());

        let hash_with_padding = Replica::checksum_prices(&prices);
        prices.insert(token, "10".to_string());
        let hash_without_padding = Replica::checksum_prices(&prices);

        assert_eq!(hash_with_padding, hash_without_padding);
    }

    fn envelope(from_sequence: u64, to_sequence: u64, events: Vec<Event>) -> Envelope {
        Envelope {
            version: 1,
            auction_id: 0,
            auction_sequence: 0,
            from_sequence,
            to_sequence,
            snapshot_sequence: None,
            events,
        }
    }

    #[test]
    fn snapshot_then_delta_updates_replica() {
        let mut replica = Replica::default();
        let uid_1 = valid_uid(1);
        let uid_2 = valid_uid(2);

        replica
            .apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 7,
                auction: RawAuctionData {
                    orders: vec![valid_order(&uid_1)],
                    prices: HashMap::from([(Address::repeat_byte(1), "100".to_string())]),
                },
            })
            .unwrap();

        replica
            .apply_delta(envelope(
                7,
                8,
                vec![
                    Event::OrderUpdated {
                        order: valid_order(&uid_1),
                    },
                    Event::OrderAdded {
                        order: valid_order(&uid_2),
                    },
                    Event::PriceChanged {
                        token: Address::repeat_byte(1),
                        price: Some("120".to_string()),
                    },
                ],
            ))
            .unwrap();

        assert_eq!(replica.sequence(), 8);
        assert_eq!(replica.orders().len(), 2);
        assert!(replica.orders().contains_key(&uid_1));
        assert_eq!(
            replica.prices().get(&Address::repeat_byte(1)).unwrap(),
            "120"
        );
    }

    fn valid_uid(byte: u8) -> String {
        format!("0x{}", hex::encode([byte; 56]))
    }

    fn uid_from_u16(value: u16) -> String {
        format!("0x{:0>112x}", value)
    }

    #[test]
    fn rejects_sequence_mismatch() {
        let mut replica = Replica::default();
        replica.apply_snapshot(snapshot(3, vec![])).unwrap();

        let err = replica.apply_delta(envelope(2, 4, vec![])).unwrap_err();

        assert!(matches!(
            err,
            Error::SequenceMismatch {
                expected: 3,
                got: 2
            }
        ));
    }

    #[test]
    fn multi_envelope_replay_matches_expected_state() {
        let mut replica = Replica::default();
        let token_a = Address::repeat_byte(0xAA);
        let token_b = Address::repeat_byte(0xBB);

        replica
            .apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 10,
                auction: RawAuctionData {
                    orders: vec![valid_order(&uid_from_u16(1)), valid_order(&uid_from_u16(2))],
                    prices: HashMap::from([
                        (token_a, "100".to_string()),
                        (token_b, "200".to_string()),
                    ]),
                },
            })
            .unwrap();

        replica
            .apply_delta(envelope(
                10,
                11,
                vec![
                    Event::OrderUpdated {
                        order: valid_order(&uid_from_u16(1)),
                    },
                    Event::PriceChanged {
                        token: token_a,
                        price: Some("150".to_string()),
                    },
                ],
            ))
            .unwrap();

        replica
            .apply_delta(envelope(
                11,
                12,
                vec![
                    Event::OrderRemoved {
                        uid: uid_from_u16(2),
                    },
                    Event::PriceChanged {
                        token: token_b,
                        price: None,
                    },
                ],
            ))
            .unwrap();

        assert_eq!(replica.sequence(), 12);
        assert_eq!(replica.orders().len(), 1);
        assert!(replica.orders().contains_key(&uid_from_u16(1)));
        assert_eq!(replica.prices().len(), 1);
        assert_eq!(replica.prices().get(&token_a).unwrap(), "150");
        assert!(!replica.prices().contains_key(&token_b));
    }

    #[test]
    fn duplicate_envelope_is_ignored() {
        let mut replica = Replica::default();
        replica
            .apply_snapshot(snapshot(5, vec![valid_order(&uid_from_u16(1))]))
            .unwrap();

        let envelope = envelope(
            5,
            6,
            vec![Event::OrderUpdated {
                order: valid_order(&uid_from_u16(1)),
            }],
        );

        replica.apply_delta(envelope.clone()).unwrap();
        replica.apply_delta(envelope).unwrap();

        assert_eq!(replica.sequence(), 6);
        assert!(replica.orders().contains_key(&uid_from_u16(1)));
    }

    #[test]
    fn stale_envelope_is_ignored() {
        let mut replica = Replica::default();
        replica
            .apply_snapshot(snapshot(8, vec![valid_order(&uid_from_u16(1))]))
            .unwrap();

        replica
            .apply_delta(envelope(
                6,
                7,
                vec![Event::OrderRemoved {
                    uid: uid_from_u16(1),
                }],
            ))
            .unwrap();

        assert_eq!(replica.sequence(), 8);
        assert!(replica.orders().contains_key(&uid_from_u16(1)));
    }

    #[test]
    fn overlapping_envelope_is_rejected() {
        let mut replica = Replica::default();
        replica.apply_snapshot(snapshot(8, vec![])).unwrap();

        let err = replica.apply_delta(envelope(7, 9, vec![])).unwrap_err();

        assert!(matches!(
            err,
            Error::SequenceMismatch {
                expected: 8,
                got: 7,
            }
        ));
    }

    #[test]
    fn snapshot_sequence_mismatch_is_rejected() {
        let mut replica = Replica::default();
        replica.apply_snapshot(snapshot(5, vec![])).unwrap();

        let err = replica
            .apply_delta(Envelope {
                version: 1,
                auction_id: 0,
                auction_sequence: 0,
                from_sequence: 5,
                to_sequence: 6,
                snapshot_sequence: Some(6),
                events: vec![],
            })
            .unwrap_err();

        assert!(matches!(
            err,
            Error::SnapshotSequenceMismatch {
                current: 5,
                snapshot: 6
            }
        ));
    }

    #[test]
    fn invalid_uid_payload_is_rejected() {
        let mut replica = Replica::default();
        let err = replica
            .apply_snapshot(snapshot(1, vec![serde_json::json!({"uid": "bad"})]))
            .unwrap_err();

        assert!(matches!(err, Error::InvalidOrderSchema(_)));
    }

    #[test]
    fn invalid_price_payload_is_rejected() {
        let mut replica = Replica::default();
        let err = replica
            .apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 1,
                auction: RawAuctionData {
                    orders: vec![],
                    prices: HashMap::from([(Address::repeat_byte(1), "not-a-number".to_string())]),
                },
            })
            .unwrap_err();

        assert!(matches!(err, Error::InvalidPriceFormat(_)));
    }

    #[test]
    fn large_batch_delta_application_converges_to_expected_state() {
        let mut replica = Replica::default();
        replica.apply_snapshot(snapshot(0, vec![])).unwrap();

        let mut events = Vec::new();
        for i in 0..600u16 {
            events.push(Event::OrderAdded {
                order: valid_order(&uid_from_u16(i)),
            });
        }
        for i in 0..300u16 {
            events.push(Event::OrderUpdated {
                order: valid_order(&uid_from_u16(i)),
            });
        }
        for i in 300..450u16 {
            events.push(Event::OrderRemoved {
                uid: uid_from_u16(i),
            });
        }
        for i in 1..=20u8 {
            events.push(Event::PriceChanged {
                token: Address::repeat_byte(i),
                price: Some((u128::from(i) * 100).to_string()),
            });
        }
        for i in 11..=20u8 {
            events.push(Event::PriceChanged {
                token: Address::repeat_byte(i),
                price: None,
            });
        }

        replica.apply_delta(envelope(0, 1, events)).unwrap();

        assert_eq!(replica.sequence(), 1);
        assert_eq!(replica.orders().len(), 450);
        assert!(replica.orders().contains_key(&uid_from_u16(0)));
        assert!(replica.orders().contains_key(&uid_from_u16(0x012b)));
        assert!(!replica.orders().contains_key(&uid_from_u16(0x012c)));
        assert_eq!(replica.prices().len(), 10);
        assert_eq!(
            replica.prices().get(&Address::repeat_byte(1)).unwrap(),
            "100"
        );
        assert!(!replica.prices().contains_key(&Address::repeat_byte(20)));
    }
}
