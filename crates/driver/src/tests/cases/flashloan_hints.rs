use {
    crate::{
        domain::competition::order::app_data::AppData,
        tests::{
            setup,
            setup::{ab_order, ab_pool, ab_solution},
        },
    },
    alloy::primitives::Address,
    app_data::{Flashloan, ProtocolAppData, hash_full_app_data},
    std::sync::Arc,
};

#[tokio::test]
#[ignore]
async fn solutions_with_flashloan() {
    let flashloan = Flashloan {
        liquidity_provider: Address::from_slice(&[1; 20]),
        receiver: Address::from_slice(&[2; 20]),
        token: Address::from_slice(&[3; 20]),
        protocol_adapter: Address::from_slice(&[4; 20]),
        amount: ::alloy::primitives::U256::from(3),
    };
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(flashloan.clone()),
        ..Default::default()
    };
    let app_data = AppData::Full(Arc::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));

    let settlement = Address::repeat_byte(5);
    let order = ab_order().app_data(app_data).receiver(Some(settlement));

    let test = setup()
        .settlement_address(settlement)
        .pool(ab_pool())
        .order(order.clone())
        // This test is just about parsing the request JSON bodies so we don't care
        // about the specific order uid here
        .solution(ab_solution().flashloan(Default::default(), flashloan_into_dto(flashloan)))
        .done()
        .await;

    // blocked by https://github.com/cowprotocol/services/issues/3218
    // todo: instead, check the solution using
    // `test.solve().await.ok().orders(&[order]);`
    test.solve().await.ok();
}

#[tokio::test]
#[ignore]
async fn solutions_without_flashloan() {
    let flashloan = Flashloan {
        liquidity_provider: Address::from_slice(&[1; 20]),
        receiver: Address::from_slice(&[2; 20]),
        token: Address::from_slice(&[3; 20]),
        protocol_adapter: Address::from_slice(&[4; 20]),
        amount: ::alloy::primitives::U256::from(3),
    };
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(flashloan.clone()),
        ..Default::default()
    };
    let app_data = AppData::Full(Arc::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let settlement = Address::repeat_byte(5);
    let order = ab_order().app_data(app_data).receiver(Some(settlement));

    let test = setup()
        .settlement_address(settlement)
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution())
        .done()
        .await;

    // blocked by https://github.com/cowprotocol/services/issues/3218
    // todo: instead, check the solution using
    // `test.solve().await.ok().orders(&[order]);`
    test.solve().await.ok();
}

fn protocol_app_data_into_validated(protocol: ProtocolAppData) -> app_data::ValidatedAppData {
    let root = app_data::Root::new(Some(protocol.clone()));
    let document = serde_json::to_string(&root).unwrap();
    let hash = app_data::AppDataHash(hash_full_app_data(document.as_bytes()));

    app_data::ValidatedAppData {
        hash,
        document,
        protocol,
    }
}

fn flashloan_into_dto(flashloan: Flashloan) -> solvers_dto::solution::Flashloan {
    solvers_dto::solution::Flashloan {
        liquidity_provider: flashloan.liquidity_provider,
        protocol_adapter: flashloan.protocol_adapter,
        receiver: flashloan.receiver,
        token: flashloan.token,
        amount: flashloan.amount,
    }
}
