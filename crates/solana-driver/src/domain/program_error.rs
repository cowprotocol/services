//! The custom error behind a failed transaction, attributed to the program
//! that raised it.

use {
    cow_settlement_interface::SettlementError,
    solana_sdk::pubkey::Pubkey,
    spl_token_interface::error::TokenError,
};

/// The Jupiter v6 aggregator program.
const JUPITER: Pubkey = Pubkey::from_str_const("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

/// Jupiter v6 error names by code, starting at 6000, as listed in its IDL:
/// <https://github.com/jup-ag/rfq-v2-sdk/blob/d776a8b1f611c12c75d72467c7cfdbb9c2193bda/fill-decoder/idls/aggregator.json>.
/// Jupiter logs no name when it fails, so its codes need a table.
const JUPITER_ERRORS: [&str; 27] = [
    "EmptyRoute",
    "SlippageToleranceExceeded",
    "InvalidCalculation",
    "MissingPlatformFeeAccount",
    "InvalidSlippage",
    "NotEnoughPercent",
    "InvalidInputIndex",
    "InvalidOutputIndex",
    "NotEnoughAccountKeys",
    "NonZeroMinimumOutAmountNotSupported",
    "InvalidRoutePlan",
    "InvalidReferralAuthority",
    "LedgerTokenAccountDoesNotMatch",
    "InvalidTokenLedger",
    "IncorrectTokenProgramID",
    "TokenProgramNotProvided",
    "SwapNotSupported",
    "ExactOutAmountNotMatched",
    "SourceAndDestinationMintCannotBeTheSame",
    "InvalidMint",
    "InvalidProgramAuthority",
    "InvalidOutputTokenAccount",
    "InvalidFeeWallet",
    "InvalidAuthority",
    "InsufficientFunds",
    "InvalidTokenAccount",
    "BondingCurveAlreadyCompleted",
];

/// A custom error code and the program that raised it, named when the code
/// is known.
#[derive(Debug, PartialEq)]
pub struct ProgramError {
    pub program: Pubkey,
    pub code: u32,
    pub name: Option<String>,
}

impl ProgramError {
    /// The custom error a failed transaction's logs report. The runtime logs
    /// `Program <id> failed: custom program error: 0x<code>` for every frame
    /// the error unwinds through, innermost first. The first line therefore
    /// names the program that raised the code, while the failing top-level
    /// instruction may belong to a caller that only passed it on.
    pub fn from_logs(settlement_program: Pubkey, logs: &[String]) -> Option<Self> {
        let (program, code) = logs.iter().find_map(|line| custom_error(line))?;
        let name = if program == settlement_program {
            SettlementError::try_from(code)
                .ok()
                .map(|err| format!("{err:?}"))
        } else if program == spl_token_interface::ID {
            TokenError::try_from(code)
                .ok()
                .map(|err| format!("{err:?}"))
        } else if program == JUPITER {
            code.checked_sub(6000)
                .and_then(|index| JUPITER_ERRORS.get(usize::try_from(index).ok()?))
                .map(|name| (*name).to_owned())
        } else {
            anchor_error_name(logs, code)
        };
        Some(Self {
            program,
            code,
            name,
        })
    }
}

/// The program and code of a `Program <id> failed: custom program error:
/// 0x<code>` line. A program's own output starts with `Program log:` or
/// `Program data:`, which never parses as an id, so it cannot forge one.
fn custom_error(line: &str) -> Option<(Pubkey, u32)> {
    let (program, code) = line
        .strip_prefix("Program ")?
        .split_once(" failed: custom program error: 0x")?;
    Some((program.parse().ok()?, u32::from_str_radix(code, 16).ok()?))
}

/// The name an Anchor program logs with error `code`, from its
/// `AnchorError ... Error Code: <name>. Error Number: <code>. ...` line.
fn anchor_error_name(logs: &[String], code: u32) -> Option<String> {
    let number = format!(". Error Number: {code}.");
    logs.iter().find_map(|line| {
        let (_, rest) = line
            .strip_prefix("Program log: AnchorError")?
            .split_once("Error Code: ")?;
        let (name, _) = rest.split_once(&number)?;
        Some(name.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTLEMENT: Pubkey =
        Pubkey::from_str_const("C7PXyLpLQBh3Ce7e9DNj3rDVUvwqa5orDwQG5hs1rfNi");
    const CLMM: Pubkey = Pubkey::from_str_const("REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2");

    fn logs(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_owned()).collect()
    }

    fn error(program: Pubkey, code: u32, name: Option<&str>) -> Option<ProgramError> {
        Some(ProgramError {
            program,
            code,
            name: name.map(str::to_owned),
        })
    }

    #[test]
    fn names_the_program_that_raised_the_code() {
        // The token transfer inside `BeginSettle` fails, and the settlement
        // program passes the token program's code on as its own.
        let token_cpi = logs(&[
            "Program C7PXyLpLQBh3Ce7e9DNj3rDVUvwqa5orDwQG5hs1rfNi invoke [1]",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
            "Program log: Error: insufficient funds",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA failed: custom program error: 0x1",
            "Program C7PXyLpLQBh3Ce7e9DNj3rDVUvwqa5orDwQG5hs1rfNi failed: custom program error: \
             0x1",
        ]);
        assert_eq!(
            ProgramError::from_logs(SETTLEMENT, &token_cpi),
            error(spl_token_interface::ID, 1, Some("InsufficientFunds"))
        );

        let own = logs(&[
            "Program C7PXyLpLQBh3Ce7e9DNj3rDVUvwqa5orDwQG5hs1rfNi invoke [1]",
            "Program C7PXyLpLQBh3Ce7e9DNj3rDVUvwqa5orDwQG5hs1rfNi failed: custom program error: \
             0x10",
        ]);
        assert_eq!(
            ProgramError::from_logs(SETTLEMENT, &own),
            error(
                SETTLEMENT,
                16,
                Some(&format!("{:?}", SettlementError::OrderExpired))
            )
        );

        let jupiter = logs(&[
            "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 invoke [1]",
            "Program log: Instruction: RouteV2",
            "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 failed: custom program error: \
             0x1771",
        ]);
        assert_eq!(
            ProgramError::from_logs(SETTLEMENT, &jupiter),
            error(JUPITER, 6001, Some("SlippageToleranceExceeded"))
        );

        let anchor_cpi = logs(&[
            "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 invoke [1]",
            "Program REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2 invoke [2]",
            "Program log: AnchorError thrown in programs/amm/src/instructions/swap.rs:194. Error \
             Code: InvalidFirstTickArrayAccount. Error Number: 6028. Error Message: Invalid first \
             tick array account.",
            "Program REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2 failed: custom program error: \
             0x178c",
            "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 failed: custom program error: \
             0x178c",
        ]);
        assert_eq!(
            ProgramError::from_logs(SETTLEMENT, &anchor_cpi),
            error(CLMM, 6028, Some("InvalidFirstTickArrayAccount"))
        );
    }

    #[test]
    fn leaves_unknown_codes_unnamed() {
        let unknown = logs(&[
            "Program log: x failed: custom program error: 0x1",
            "Program REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2 failed: custom program error: \
             0x2a",
        ]);
        assert_eq!(
            ProgramError::from_logs(SETTLEMENT, &unknown),
            error(CLMM, 42, None)
        );

        let not_custom = logs(&[
            "Program C7PXyLpLQBh3Ce7e9DNj3rDVUvwqa5orDwQG5hs1rfNi failed: insufficient account \
             keys for instruction",
        ]);
        assert_eq!(ProgramError::from_logs(SETTLEMENT, &not_custom), None);
    }
}
