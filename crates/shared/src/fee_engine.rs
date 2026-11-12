//! An order's app data as the input of the fee engine, for the services that
//! host it.

use {
    app_data::{FeePolicy, ProtocolAppData},
    fee_engine::PartnerFee,
    model::order::OrderUid,
};

/// The app data of an order as the rules see it.
pub fn app_data(protocol: &ProtocolAppData, app_code: Option<String>) -> fee_engine::AppData {
    fee_engine::AppData {
        app_code,
        signer: protocol.signer,
        replaced_order: protocol
            .replaced_order
            .as_ref()
            .map(|replaced| OrderUid(replaced.uid.0)),
        flashloan: protocol
            .flashloan
            .as_ref()
            .map(|flashloan| fee_engine::Flashloan {
                liquidity_provider: flashloan.liquidity_provider,
                protocol_adapter: flashloan.protocol_adapter,
                receiver: flashloan.receiver,
                token: flashloan.token,
                amount: flashloan.amount,
            }),
    }
}

/// The partner fees of the app data, for the engine to cap.
pub fn partner_fees(protocol: &ProtocolAppData) -> Vec<PartnerFee> {
    protocol
        .partner_fee
        .iter()
        .map(|fee| match fee.policy {
            FeePolicy::Volume { bps } => PartnerFee::Volume { bps },
            FeePolicy::Surplus {
                bps,
                max_volume_bps,
            } => PartnerFee::Surplus {
                bps,
                max_volume_bps,
            },
            FeePolicy::PriceImprovement {
                bps,
                max_volume_bps,
            } => PartnerFee::PriceImprovement {
                bps,
                max_volume_bps,
            },
        })
        .collect()
}

/// The partner fees a fast-path order pays. It settles at the quote, so only
/// partner volume fees charge anything; the others must not take the cap's
/// room.
pub fn fast_path_partner_fees(protocol: &ProtocolAppData) -> Vec<PartnerFee> {
    partner_fees(protocol)
        .into_iter()
        .filter(|fee| matches!(fee, PartnerFee::Volume { .. }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_path_keeps_only_partner_volume_fees() {
        let protocol = app_data::parse(
            br#"{"metadata": {"partnerFee": [
                {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 50, "maxVolumeBps": 80},
                {"recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(
            partner_fees(&protocol),
            [
                PartnerFee::Surplus {
                    bps: 50,
                    max_volume_bps: 80
                },
                PartnerFee::Volume { bps: 50 },
            ]
        );
        assert_eq!(
            fast_path_partner_fees(&protocol),
            [PartnerFee::Volume { bps: 50 }]
        );
    }
}
