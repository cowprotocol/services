use super::*;

#[test]
fn gas_estimator_alloy_defaults() {
    let config: GasEstimatorType = toml::from_str(
        r#"
            estimator = "alloy"
        "#,
    )
    .unwrap();

    match config {
        GasEstimatorType::Alloy {
            past_blocks,
            reward_percentile,
        } => {
            assert_eq!(past_blocks, 10);
            assert_eq!(reward_percentile, 20.0);
        }
        _ => panic!("expected Alloy variant"),
    }
}

#[test]
fn gas_estimator_alloy_custom_past_blocks() {
    let config: GasEstimatorType = toml::from_str(
        r#"
            estimator = "alloy"
            past-blocks = 5
        "#,
    )
    .unwrap();

    match config {
        GasEstimatorType::Alloy {
            past_blocks,
            reward_percentile,
        } => {
            assert_eq!(past_blocks, 5);
            assert_eq!(reward_percentile, 20.0);
        }
        _ => panic!("expected Alloy variant"),
    }
}

#[test]
fn gas_estimator_alloy_custom_percentile() {
    let config: GasEstimatorType = toml::from_str(
        r#"
            estimator = "alloy"
            reward-percentile = 50.0
        "#,
    )
    .unwrap();

    match config {
        GasEstimatorType::Alloy {
            past_blocks,
            reward_percentile,
        } => {
            assert_eq!(past_blocks, 10);
            assert_eq!(reward_percentile, 50.0);
        }
        _ => panic!("expected Alloy variant"),
    }
}

#[test]
fn gas_estimator_alloy_all_custom() {
    let config: GasEstimatorType = toml::from_str(
        r#"
            estimator = "alloy"
            past-blocks = 20
            reward-percentile = 75.0
        "#,
    )
    .unwrap();

    match config {
        GasEstimatorType::Alloy {
            past_blocks,
            reward_percentile,
        } => {
            assert_eq!(past_blocks, 20);
            assert_eq!(reward_percentile, 75.0);
        }
        _ => panic!("expected Alloy variant"),
    }
}

#[test]
fn gas_estimator_web3() {
    let config: GasEstimatorType = toml::from_str(
        r#"
            estimator = "web3"
        "#,
    )
    .unwrap();

    assert!(matches!(config, GasEstimatorType::Web3));
}

#[test]
fn gas_estimator_default() {
    let config = GasEstimatorType::default();

    match config {
        GasEstimatorType::Alloy {
            past_blocks,
            reward_percentile,
        } => {
            assert_eq!(past_blocks, 10);
            assert_eq!(reward_percentile, 20.0);
        }
        _ => panic!("expected Alloy variant as default"),
    }
}
