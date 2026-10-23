//! Proof that an order came through a partner.
//!
//! `appCode` is self-declared, so a discount rule must not trust it. A partner
//! that wants discounts has its backend sign a [`PartnerAttestation`] over the
//! order and puts the signature in `appData.metadata.partner.signature`. The
//! engine recovers the signer and checks it against the partner's registered
//! signers.

use {
    alloy_primitives::{Address, B256, Signature, hex},
    alloy_sol_types::{Eip712Domain, SolStruct, eip712_domain, sol},
    model::order::{Order, OrderKind},
    std::collections::{HashMap, HashSet},
};

sol! {
    /// Every signed order field except the app data, which holds the
    /// signature itself. The order already commits to the app-data hash, so
    /// the two bind each other without a loop.
    #[derive(Debug, PartialEq, Eq)]
    struct PartnerAttestation {
        address owner;
        address sellToken;
        address buyToken;
        address receiver;
        uint256 sellAmount;
        uint256 buyAmount;
        uint32 validTo;
        uint256 feeAmount;
        bytes32 kind;
        bool partiallyFillable;
        bytes32 sellTokenBalance;
        bytes32 buyTokenBalance;
    }
}

impl PartnerAttestation {
    pub fn for_order(order: &Order) -> Self {
        let kind = match order.data.kind {
            OrderKind::Buy => OrderKind::BUY,
            OrderKind::Sell => OrderKind::SELL,
        };
        Self {
            owner: order.metadata.owner,
            sellToken: order.data.sell_token,
            buyToken: order.data.buy_token,
            receiver: Address::from(*order.data.receiver.raw_bytes()),
            sellAmount: order.data.sell_amount,
            buyAmount: order.data.buy_amount,
            validTo: order.data.valid_to,
            feeAmount: order.data.fee_amount,
            kind: kind.into(),
            partiallyFillable: order.data.partially_fillable,
            sellTokenBalance: order.data.sell_token_balance.as_bytes().into(),
            buyTokenBalance: order.data.buy_token_balance.as_bytes().into(),
        }
    }

    /// The EIP-712 hash a partner signs.
    pub fn signing_hash(&self, chain_id: u64) -> B256 {
        self.eip712_signing_hash(&domain(chain_id))
    }
}

/// The EIP-712 domain attestations are signed under.
pub fn domain(chain_id: u64) -> Eip712Domain {
    eip712_domain! {
        name: "CoW Protocol Partner Attestation",
        version: "1",
        chain_id: chain_id,
    }
}

/// Registered signers by partner app code.
pub(crate) type Signers = HashMap<String, HashSet<Address>>;

/// The app code the order proves, or `None` when it carries no attestation
/// from one of that app code's registered signers.
pub(crate) fn verified_partner(
    order: &Order,
    app_data: &serde_json::Value,
    signers: &Signers,
    chain_id: u64,
) -> Option<String> {
    let app_code = app_data.get("appCode")?.as_str()?;
    let registered = signers.get(app_code)?;
    let signature = app_data.pointer("/metadata/partner/signature")?.as_str()?;
    let signature = Signature::from_raw(&hex::decode(signature).ok()?).ok()?;
    let hash = PartnerAttestation::for_order(order).signing_hash(chain_id);
    let signer = signature.recover_address_from_prehash(&hash).ok()?;
    registered.contains(&signer).then(|| app_code.to_owned())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{U256, address},
        alloy_signer::SignerSync,
        alloy_signer_local::PrivateKeySigner,
        app_data::document_view,
        model::order::{OrderData, OrderMetadata},
        serde_json::json,
    };

    const CHAIN_ID: u64 = 1;

    fn sample() -> Order {
        Order {
            metadata: OrderMetadata {
                owner: address!("0x1111111111111111111111111111111111111111"),
                ..Default::default()
            },
            data: OrderData {
                sell_token: address!("0x2222222222222222222222222222222222222222"),
                buy_token: address!("0x3333333333333333333333333333333333333333"),
                sell_amount: U256::from(1_000_000u64),
                buy_amount: U256::from(900_000u64),
                valid_to: 1_800_000_000,
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn attested(order: &mut Order, signer: &PrivateKeySigner, app_code: &str) {
        let hash = PartnerAttestation::for_order(order).signing_hash(CHAIN_ID);
        let signature = signer.sign_hash_sync(&hash).unwrap();
        order.metadata.full_app_data = Some(
            json!({
                "appCode": app_code,
                "metadata": { "partner": { "signature": hex::encode_prefixed(signature.as_bytes()) } },
            })
            .to_string(),
        );
    }

    fn verify(order: &Order, signers: &Signers) -> Option<String> {
        let app_data = document_view(order.metadata.full_app_data.as_deref());
        verified_partner(order, &app_data, signers, CHAIN_ID)
    }

    #[test]
    fn accepts_a_registered_signer() {
        let signer = PrivateKeySigner::random();
        let signers = Signers::from([("acme".to_owned(), HashSet::from([signer.address()]))]);
        let mut order = sample();
        attested(&mut order, &signer, "acme");
        assert_eq!(verify(&order, &signers).as_deref(), Some("acme"));
    }

    #[test]
    fn rejects_unregistered_signers_other_app_codes_and_changed_orders() {
        let signer = PrivateKeySigner::random();
        let signers = Signers::from([("acme".to_owned(), HashSet::from([signer.address()]))]);

        let mut order = sample();
        attested(&mut order, &PrivateKeySigner::random(), "acme");
        assert_eq!(verify(&order, &signers), None);

        let mut order = sample();
        attested(&mut order, &signer, "beta");
        assert_eq!(verify(&order, &signers), None);

        let mut order = sample();
        attested(&mut order, &signer, "acme");
        order.data.sell_amount += U256::from(1);
        assert_eq!(verify(&order, &signers), None);

        let mut order = sample();
        attested(&mut order, &signer, "acme");
        assert_eq!(
            verified_partner(
                &order,
                &document_view(order.metadata.full_app_data.as_deref()),
                &signers,
                CHAIN_ID + 1
            ),
            None
        );
    }

    #[test]
    fn ignores_orders_without_an_attestation() {
        let signers = Signers::from([("acme".to_owned(), HashSet::new())]);
        let mut order = sample();
        assert_eq!(verify(&order, &signers), None);
        order.metadata.full_app_data = Some(r#"{"appCode":"acme"}"#.to_owned());
        assert_eq!(verify(&order, &signers), None);
        order.metadata.full_app_data =
            Some(r#"{"appCode":"acme","metadata":{"partner":{"signature":"0x1234"}}}"#.to_owned());
        assert_eq!(verify(&order, &signers), None);
    }
}
