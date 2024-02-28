use {
    crate::interaction::EncodedInteraction,
    anyhow::Result,
    ethcontract::Bytes,
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::{Signature, SigningScheme},
    },
    primitive_types::{H160, U256},
};

pub type EncodedTrade = (
    U256,            // sellTokenIndex
    U256,            // buyTokenIndex
    H160,            // receiver
    U256,            // sellAmount
    U256,            // buyAmount
    u32,             // validTo
    Bytes<[u8; 32]>, // appData
    U256,            // feeAmount
    U256,            // flags
    U256,            // executedAmount
    Bytes<Vec<u8>>,  // signature
);

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderData,
    signature: &Signature,
    owner: H160,
    sell_token_index: usize,
    buy_token_index: usize,
    executed_amount: &U256,
) -> EncodedTrade {
    (
        sell_token_index.into(),
        buy_token_index.into(),
        order.receiver.unwrap_or_else(H160::zero),
        order.sell_amount,
        order.buy_amount,
        order.valid_to,
        Bytes(order.app_data.0),
        order.fee_amount,
        order_flags(order, signature),
        *executed_amount,
        Bytes(signature.encode_for_settlement(owner).to_vec()),
    )
}

fn order_flags(order: &OrderData, signature: &Signature) -> U256 {
    let mut result = 0u8;
    // The kind is encoded as 1 bit in position 0.
    result |= match order.kind {
        OrderKind::Sell => 0b0,
        OrderKind::Buy => 0b1,
    };
    // The order fill kind is encoded as 1 bit in position 1.
    result |= (order.partially_fillable as u8) << 1;
    // The order sell token balance is encoded as 2 bits in position 2.
    result |= match order.sell_token_balance {
        SellTokenSource::Erc20 => 0b00,
        SellTokenSource::External => 0b10,
        SellTokenSource::Internal => 0b11,
    } << 2;
    // The order buy token balance is encoded as 1 bit in position 4.
    result |= match order.buy_token_balance {
        BuyTokenDestination::Erc20 => 0b0,
        BuyTokenDestination::Internal => 0b1,
    } << 4;
    // The signing scheme is encoded as a 2 bits in position 5.
    result |= match signature.scheme() {
        SigningScheme::Eip712 => 0b00,
        SigningScheme::EthSign => 0b01,
        SigningScheme::Eip1271 => 0b10,
        SigningScheme::PreSign => 0b11,
    } << 5;
    result.into()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<H160>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: [Vec<EncodedInteraction>; 3],
}

impl EncodedSettlement {
    /// Order uids for all trades in the settlement.
    ///
    /// Returns all order uids or none.
    pub fn uids(
        &self,
        domain_separator: model::DomainSeparator,
    ) -> Result<Vec<model::order::OrderUid>> {
        self.trades
            .iter()
            .map(|trade| {
                let order = model::order::OrderData {
                    sell_token: self.tokens[trade.0.as_u64() as usize],
                    buy_token: self.tokens[trade.1.as_u64() as usize],
                    sell_amount: trade.3,
                    buy_amount: trade.4,
                    valid_to: trade.5,
                    app_data: model::app_data::AppDataHash(trade.6 .0),
                    fee_amount: trade.7,
                    kind: if trade.8.byte(0) & 0b1 == 0 {
                        model::order::OrderKind::Sell
                    } else {
                        model::order::OrderKind::Buy
                    },
                    partially_fillable: trade.8.byte(0) & 0b10 != 0,
                    receiver: Some(trade.2),
                    sell_token_balance: if trade.8.byte(0) & 0x08 == 0 {
                        model::order::SellTokenSource::Erc20
                    } else if trade.8.byte(0) & 0x04 == 0 {
                        model::order::SellTokenSource::External
                    } else {
                        model::order::SellTokenSource::Internal
                    },
                    buy_token_balance: if trade.8.byte(0) & 0x10 == 0 {
                        model::order::BuyTokenDestination::Erc20
                    } else {
                        model::order::BuyTokenDestination::Internal
                    },
                };
                let signing_scheme = match trade.8.byte(0) >> 5 {
                    0b00 => model::signature::SigningScheme::Eip712,
                    0b01 => model::signature::SigningScheme::EthSign,
                    0b10 => model::signature::SigningScheme::Eip1271,
                    0b11 => model::signature::SigningScheme::PreSign,
                    _ => unreachable!(),
                };
                let signature = Signature::from_bytes(signing_scheme, &trade.10 .0)?;
                let owner = signature.recover_owner(
                    &signature.to_bytes(),
                    &domain_separator,
                    &order.hash_struct(),
                )?;
                Ok(order.uid(&domain_separator, &owner))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ethcontract::H256, hex_literal::hex, model::signature::EcdsaSignature};

    #[test]
    fn order_flag_permutations() {
        for (order, scheme, flags) in [
            (
                OrderData {
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Erc20,
                    ..Default::default()
                },
                SigningScheme::Eip712,
                // ......0 - sell order
                // .....0. - fill-or-kill order
                // ...00.. - ERC20 sell token balance
                // ..0.... - ERC20 buy token balance
                // 00..... - EIP-712 signing scheme
                0b0000000,
            ),
            (
                OrderData {
                    kind: OrderKind::Sell,
                    partially_fillable: true,
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Internal,
                    ..Default::default()
                },
                SigningScheme::Eip1271,
                // ......0 - sell order
                // .....1. - partially fillable order
                // ...00.. - ERC20 sell token balance
                // ..1.... - Vault-internal buy token balance
                // 10..... - EIP-712 signing scheme
                0b1010010,
            ),
            (
                OrderData {
                    kind: OrderKind::Buy,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::External,
                    buy_token_balance: BuyTokenDestination::Erc20,
                    ..Default::default()
                },
                SigningScheme::PreSign,
                // ......1 - buy order
                // .....0. - fill-or-kill order
                // ...10.. - Vault-external sell token balance
                // ..0.... - ERC20 buy token balance
                // 11..... - Pre-sign signing scheme
                0b1101001,
            ),
            (
                OrderData {
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::Internal,
                    buy_token_balance: BuyTokenDestination::Erc20,
                    ..Default::default()
                },
                SigningScheme::EthSign,
                // ......0 - sell order
                // .....0. - fill-or-kill order
                // ...11.. - Vault-internal sell token balance
                // ..0.... - ERC20 buy token balance
                // 01..... - Eth-sign signing scheme
                0b0101100,
            ),
            (
                OrderData {
                    kind: OrderKind::Buy,
                    partially_fillable: true,
                    sell_token_balance: SellTokenSource::Internal,
                    buy_token_balance: BuyTokenDestination::Internal,
                    ..Default::default()
                },
                SigningScheme::PreSign,
                // ......1 - buy order
                // .....1. - partially fillable order
                // ...11.. - Vault-internal sell token balance
                // ..1.... - Vault-internal buy token balance
                // 11..... - Pre-sign signing scheme
                0b1111111,
            ),
        ] {
            assert_eq!(
                order_flags(&order, &Signature::default_with(scheme)),
                U256::from(flags)
            );
        }
    }

    #[test]
    fn trade_signature_encoding() {
        let owner = H160([1; 20]);
        for (signature, bytes) in [
            (Signature::Eip712(Default::default()), vec![0; 65]),
            (
                Signature::EthSign(EcdsaSignature {
                    r: H256([1; 32]),
                    s: H256([1; 32]),
                    v: 1,
                }),
                vec![1; 65],
            ),
            (
                Signature::Eip1271(vec![1, 2, 3, 4]),
                hex!(
                    "0101010101010101010101010101010101010101"
                    "01020304"
                )
                .to_vec(),
            ),
            (Signature::PreSign, vec![1; 20]),
        ] {
            let (.., encoded_signature) = encode_trade(
                &Default::default(),
                &signature,
                owner,
                Default::default(),
                Default::default(),
                &Default::default(),
            );
            assert_eq!(encoded_signature.0, bytes);
        }
    }
}
