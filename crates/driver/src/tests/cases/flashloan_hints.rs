use {
    crate::{
        domain::competition::order::app_data::AppData,
        tests::{
            setup,
            setup::{ab_order, ab_pool, ab_solution},
        },
    },
    app_data::{Flashloan, ProtocolAppData, hash_full_app_data},
    primitive_types::H160,
};

#[tokio::test]
#[ignore]
async fn solutions_with_flashloan() {
    let flashloan = Flashloan {
        lender: Some(H160::from_low_u64_be(1)),
        borrower: Some(H160::from_low_u64_be(2)),
        token: H160::from_low_u64_be(3),
        amount: 3.into(),
    };
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(flashloan.clone()),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));

    let settlement = H160([5; 20]);
    let order = ab_order().app_data(app_data).receiver(Some(settlement));

    let test = setup()
        .settlement_address(&settlement)
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution().flashloan(flashloan_into_dto(flashloan)))
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
        lender: Some(H160::from_low_u64_be(1)),
        borrower: Some(H160::from_low_u64_be(2)),
        token: H160::from_low_u64_be(3),
        amount: 3.into(),
    };
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(flashloan.clone()),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let settlement = H160([5; 20]);
    let order = ab_order().app_data(app_data).receiver(Some(settlement));

    let test = setup()
        .settlement_address(&settlement)
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
        lender: flashloan.lender.unwrap_or_default(),
        borrower: flashloan.borrower.unwrap_or_default(),
        token: flashloan.token,
        amount: flashloan.amount,
    }
}
