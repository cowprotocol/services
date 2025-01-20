use {
    crate::{
        domain::competition::order::AppData,
        infra::solver::dto::FlashloanLender,
        tests::{
            setup,
            setup::{ab_order, ab_pool, ab_solution, protocol_app_data_into_validated},
        },
    },
    app_data::{Flashloan, ProtocolAppData},
    primitive_types::H160,
};

#[tokio::test]
#[ignore]
async fn valid_data() {
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(Flashloan {
            lender: Some(H160::from_low_u64_be(1)),
            borrower: Some(H160::from_low_u64_be(2)),
            token: H160::from_low_u64_be(3),
            amount: 3.into(),
        }),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let order = ab_order().app_data(app_data);

    let test = setup()
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

#[tokio::test]
#[ignore]
async fn missing_flashloan_solution() {
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(Flashloan {
            borrower: Some(H160::from_low_u64_be(1)),
            token: H160::from_low_u64_be(3),
            amount: 3.into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let order = ab_order().app_data(app_data);

    let test = setup()
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution().flashloan_lender(order.name, None))
        .done()
        .await;

    test.solve().await.err().kind("SolverFailed");
}

#[tokio::test]
#[ignore]
async fn wrong_flashloan_lender_address_solution() {
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(Flashloan {
            borrower: Some(H160::from_low_u64_be(1)),
            token: H160::from_low_u64_be(3),
            amount: 3.into(),
            lender: Some(H160::from_low_u64_be(2)),
        }),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let order = ab_order().app_data(app_data);

    let test = setup()
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution().flashloan_lender(
            order.name,
            Some(FlashloanLender {
                address: H160::from_low_u64_be(100),
                token: H160::from_low_u64_be(3),
                amount: 3.into(),
            }),
        ))
        .done()
        .await;

    test.solve().await.err().kind("SolverFailed");
}

#[tokio::test]
#[ignore]
async fn wrong_flashloan_lender_token_solution() {
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(Flashloan {
            borrower: Some(H160::from_low_u64_be(1)),
            token: H160::from_low_u64_be(3),
            amount: 3.into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let order = ab_order().app_data(app_data);

    let test = setup()
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution().flashloan_lender(
            order.name,
            Some(FlashloanLender {
                address: H160::from_low_u64_be(1),
                token: H160::from_low_u64_be(2),
                amount: 3.into(),
            }),
        ))
        .done()
        .await;

    test.solve().await.err().kind("SolverFailed");
}

#[tokio::test]
#[ignore]
async fn insufficient_flashloan_lender_amount_solution() {
    let protocol_app_data = ProtocolAppData {
        flashloan: Some(Flashloan {
            borrower: Some(H160::from_low_u64_be(1)),
            token: H160::from_low_u64_be(3),
            amount: 3.into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let app_data = AppData::Full(Box::new(protocol_app_data_into_validated(
        protocol_app_data,
    )));
    let order = ab_order().app_data(app_data);

    let test = setup()
        .pool(ab_pool())
        .order(order.clone())
        .solution(ab_solution().flashloan_lender(
            order.name,
            Some(FlashloanLender {
                address: H160::from_low_u64_be(1),
                token: H160::from_low_u64_be(3),
                amount: 1.into(),
            }),
        ))
        .done()
        .await;

    test.solve().await.err().kind("SolverFailed");
}
