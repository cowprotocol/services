use {super::*, alloy::primitives::address, std::num::NonZeroUsize};

const CALLER_A: Address = address!("0000000000000000000000000000000000000001");
const CALLER_B: Address = address!("0000000000000000000000000000000000000002");
const CALLER_C: Address = address!("0000000000000000000000000000000000000003");
const CALLER_D: Address = address!("0000000000000000000000000000000000000004");
const CALLER_E: Address = address!("0000000000000000000000000000000000000005");
const CALLER_F: Address = address!("0000000000000000000000000000000000000006");

#[test]
fn delegate_target_is_stable_and_caller_sensitive() {
    let first = DelegateDeployment::new(&[CALLER_A, CALLER_B]).unwrap();
    let same = DelegateDeployment::new(&[CALLER_A, CALLER_B]).unwrap();
    let reordered = DelegateDeployment::new(&[CALLER_B, CALLER_A]).unwrap();

    assert_eq!(first.target, same.target);
    assert_ne!(first.target, reordered.target);
}

#[test]
fn pads_approved_callers_to_contract_capacity() {
    let deployment = DelegateDeployment::new(&[CALLER_A, CALLER_B]).unwrap();

    assert_eq!(
        deployment.approved_callers,
        [
            CALLER_A,
            CALLER_B,
            Address::ZERO,
            Address::ZERO,
            Address::ZERO
        ]
    );
}

#[test]
fn rejects_more_callers_than_the_delegate_supports() {
    let err =
        DelegateDeployment::new(&[CALLER_A, CALLER_B, CALLER_C, CALLER_D, CALLER_E, CALLER_F])
            .unwrap_err();

    assert!(err.to_string().contains("at most 5"));
}

#[test]
fn rejects_read_only_submission_accounts() {
    let err = validate_config(
        "solver",
        NonZeroUsize::new(2).unwrap(),
        &[Account::Address(CALLER_A), Account::Address(CALLER_B)],
    )
    .unwrap_err();

    assert!(err.to_string().contains("must be signers"));
}

#[test]
fn rejects_multiple_proposed_solutions_without_submission_accounts() {
    let err = validate_config("solver", NonZeroUsize::new(2).unwrap(), &[]).unwrap_err();

    assert!(err.to_string().contains("max-solutions-to-propose > 1"));
}

#[test]
fn accepts_single_solution_without_submission_accounts() {
    let deployment = validate_config("solver", NonZeroUsize::new(1).unwrap(), &[]).unwrap();

    assert!(deployment.is_none());
}

#[test]
fn detects_eip7702_delegation_target() {
    let delegate = address!("0000000000000000000000000000000000000007");
    let other = address!("0000000000000000000000000000000000000008");
    let mut code = Vec::from(DELEGATION_PREFIX);
    code.extend_from_slice(delegate.as_slice());

    assert!(is_delegated_to(&code, delegate));
    assert!(!is_delegated_to(&code, other));
}
