use {
    alloy::contract::{CallBuilder, CallDecoder},
    app_data::Hook,
    ethrpc::{AlloyProvider, alloy::conversions::IntoLegacy},
};

pub async fn hook_for_transaction<D>(tx: CallBuilder<&AlloyProvider, D>) -> anyhow::Result<Hook>
where
    D: CallDecoder,
{
    let gas_limit = tx.estimate_gas().await?;
    let call_data = tx.calldata().to_vec();
    let target = tx
        .into_transaction_request()
        .to
        .unwrap()
        .into_to()
        .unwrap()
        .into_legacy();

    Ok(Hook {
        target,
        call_data,
        gas_limit,
    })
}
