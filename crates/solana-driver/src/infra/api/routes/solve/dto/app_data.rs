//! Parsing of app_data for purposes of solver encoding.

use {
    crate::domain::hooks::{Hook, HookAccount, Hooks},
    serde::Deserialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
};

/// Why an order's app data hooks are invalid.
#[derive(Debug, thiserror::Error)]
pub enum InvalidHooks {
    #[error("app data hooks do not parse: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("hook calling {program} consumes signer account {account}")]
    ConsumesSigner { program: Pubkey, account: Pubkey },
}

/// The hooks the app data `document` names. A document without hooks names
/// none, like a missing one.
pub fn hooks(document: Option<&str>) -> Result<Hooks, InvalidHooks> {
    let Some(document) = document else {
        return Ok(Hooks::default());
    };
    let Document { metadata } = serde_json::from_str(document)?;
    let Some(hooks) = metadata.and_then(|metadata| metadata.hooks) else {
        return Ok(Hooks::default());
    };
    Ok(Hooks {
        pre: hooks
            .pre
            .into_iter()
            .map(Hook::try_from)
            .collect::<Result<_, _>>()?,
        post: hooks
            .post
            .into_iter()
            .map(Hook::try_from)
            .collect::<Result<_, _>>()?,
    })
}

#[derive(Deserialize)]
struct Document {
    #[serde(default)]
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(default)]
    hooks: Option<HooksDto>,
}

#[derive(Deserialize)]
struct HooksDto {
    #[serde(default)]
    pre: Vec<HookDto>,
    #[serde(default)]
    post: Vec<HookDto>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookDto {
    #[serde_as(as = "DisplayFromStr")]
    target: Pubkey,
    #[serde(deserialize_with = "call_data")]
    call_data: Vec<u8>,
    #[serde(default)]
    accounts: Vec<AccountDto>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountDto {
    #[serde_as(as = "DisplayFromStr")]
    pubkey: Pubkey,
    #[serde(default)]
    is_signer: bool,
    is_writable: bool,
}

impl TryFrom<HookDto> for Hook {
    type Error = InvalidHooks;

    fn try_from(hook: HookDto) -> Result<Self, Self::Error> {
        let accounts = hook
            .accounts
            .into_iter()
            .map(|account| {
                if account.is_signer {
                    return Err(InvalidHooks::ConsumesSigner {
                        program: hook.target,
                        account: account.pubkey,
                    });
                }
                Ok(HookAccount {
                    pubkey: account.pubkey,
                    is_writable: account.is_writable,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            program_id: hook.target,
            accounts,
            data: hook.call_data,
        })
    }
}

/// `0x`-prefixed hex, as the schema's `callData` pattern `^0x[a-fA-F0-9]*$`.
fn call_data<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let hex = String::deserialize(deserializer)?;
    let digits = hex
        .strip_prefix("0x")
        .ok_or_else(|| serde::de::Error::custom("callData must be 0x-prefixed hex"))?;
    const_hex::decode(digits).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    #[test]
    fn a_missing_document_or_hooks_name_none() {
        for document in [
            None,
            Some("{}"),
            Some(r#"{"metadata": {}}"#),
            Some(r#"{"metadata": null}"#),
            Some(r#"{"appCode": "x", "metadata": {"hooks": null, "utm": {}}}"#),
            Some(r#"{"metadata": {"hooks": {}}}"#),
        ] {
            assert_eq!(hooks(document).unwrap(), Hooks::default(), "{document:?}");
        }
    }

    #[test]
    fn parses_pre_and_post_hooks() {
        let document = serde_json::json!({
            "appCode": "CoW Swap",
            "metadata": {"hooks": {
                "version": "0.3.0",
                "pre": [{
                    "target": pubkey(1).to_string(),
                    "callData": "0xdeadbeef",
                    "accounts": [
                        {"pubkey": pubkey(2).to_string(), "isSigner": false, "isWritable": true},
                        {"pubkey": pubkey(3).to_string(), "isWritable": false},
                    ],
                    "dappId": "some-dapp",
                }],
                "post": [{
                    "target": pubkey(4).to_string(),
                    "callData": "0x",
                    "gasLimit": "100000",
                }],
            }},
        })
        .to_string();
        assert_eq!(
            hooks(Some(&document)).unwrap(),
            Hooks {
                pre: vec![Hook {
                    program_id: pubkey(1),
                    accounts: vec![
                        HookAccount {
                            pubkey: pubkey(2),
                            is_writable: true,
                        },
                        HookAccount {
                            pubkey: pubkey(3),
                            is_writable: false,
                        },
                    ],
                    data: vec![0xde, 0xad, 0xbe, 0xef],
                }],
                post: vec![Hook {
                    program_id: pubkey(4),
                    accounts: Vec::new(),
                    data: Vec::new(),
                }],
            }
        );
    }

    #[test]
    fn a_hook_consuming_a_signer_is_invalid() {
        let document = serde_json::json!({"metadata": {"hooks": {"post": [{
            "target": pubkey(1).to_string(),
            "callData": "0x00",
            "accounts": [{"pubkey": pubkey(2).to_string(), "isSigner": true, "isWritable": false}],
        }]}}})
        .to_string();
        assert!(matches!(
            hooks(Some(&document)),
            Err(InvalidHooks::ConsumesSigner { program, account })
                if program == pubkey(1) && account == pubkey(2)
        ));
    }

    #[test]
    fn malformed_hooks_are_invalid() {
        for hook in [
            // An EVM target.
            serde_json::json!({"target": "0x0000000000000000000000000000000000000001", "callData": "0x"}),
            // No 0x prefix.
            serde_json::json!({"target": pubkey(1).to_string(), "callData": "dead"}),
            // Odd hex.
            serde_json::json!({"target": pubkey(1).to_string(), "callData": "0xabc"}),
            // No call data.
            serde_json::json!({"target": pubkey(1).to_string()}),
            // An account without its writable flag.
            serde_json::json!({
                "target": pubkey(1).to_string(),
                "callData": "0x",
                "accounts": [{"pubkey": pubkey(2).to_string()}],
            }),
        ] {
            let document = serde_json::json!({"metadata": {"hooks": {"pre": [hook]}}}).to_string();
            assert!(
                matches!(hooks(Some(&document)), Err(InvalidHooks::Malformed(_))),
                "{document}"
            );
        }
        assert!(matches!(
            hooks(Some("not json")),
            Err(InvalidHooks::Malformed(_))
        ));
    }
}
