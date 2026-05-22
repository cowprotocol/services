use {
    super::*,
    alloy::primitives::{Bytes, address},
};

const ORIGINAL_FROM: eth::Address = address!("0000000000000000000000000000000000000001");
const SETTLEMENT: eth::Address = address!("0000000000000000000000000000000000000002");
const SOLVER: eth::Address = address!("0000000000000000000000000000000000000003");
const SUBMITTER: eth::Address = address!("0000000000000000000000000000000000000004");

fn tx(input: Bytes) -> eth::Tx {
    eth::Tx {
        from: ORIGINAL_FROM,
        to: SETTLEMENT,
        value: 0.into(),
        input,
        access_list: Default::default(),
    }
}

#[test]
fn delegated_submission_rewrites_transaction() {
    let prepared = prepare_submission(
        &tx(Bytes::from_static(&[0xaa, 0xbb])),
        &SubmissionMode::Delegated {
            submitter_eoa: SUBMITTER,
            solver_eoa: SOLVER,
        },
    );
    let mut expected = SETTLEMENT.as_slice().to_vec();
    expected.extend_from_slice(&[0xaa, 0xbb]);

    assert_eq!(prepared.from, SUBMITTER);
    assert_eq!(prepared.to, SOLVER);
    assert_eq!(prepared.input, Bytes::from(expected));
}
