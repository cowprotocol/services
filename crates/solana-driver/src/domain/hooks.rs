//! User hooks: instructions an order's app data asks the settlement to run
//! around its fill, the Solana counterpart of the EVM pre- and post-hooks.

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// The hooks of one order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hooks {
    /// Run before `BeginSettle` pulls the order's sell tokens, on the order's
    /// first fill only.
    pub pre: Vec<Hook>,
    /// Run after `FinalizeSettle` pays the order out, on every fill.
    pub post: Vec<Hook>,
}

/// One hook instruction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hook {
    /// The program the instruction calls: the hook's `target`.
    pub program_id: Pubkey,
    /// The accounts the instruction takes, in order.
    pub accounts: Vec<HookAccount>,
    /// The instruction data
    pub data: Vec<u8>,
}

/// An account a hook instruction takes. There is no signer flag because a hook
/// should always be publicly executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HookAccount {
    pub pubkey: Pubkey,
    pub is_writable: bool,
}

/// Why an order's hooks may not run in a settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidHook {
    /// The hook lists the settling solver, the settlement's signer, and would
    /// act with its authority.
    #[error("hook calling {program} consumes the settling solver's signer account")]
    ConsumesSolverSigner { program: Pubkey },
}

impl Hooks {
    pub fn is_empty(&self) -> bool {
        self.pre.is_empty() && self.post.is_empty()
    }

    fn iter(&self) -> impl Iterator<Item = &Hook> {
        self.pre.iter().chain(&self.post)
    }

    /// Check that the hooks may run in a settlement signed by `solver`: none
    /// consumes the solver's account, which signs the settlement.
    pub fn check(&self, solver: &Pubkey) -> Result<(), InvalidHook> {
        self.iter().try_for_each(|hook| hook.check(solver))
    }

    /// The instructions to add to an order for these hooks
    pub fn instructions(
        &self,
        solver: &Pubkey,
        fill_count: u64,
    ) -> Result<HookInstructions, InvalidHook> {
        // Check every hook, even the pre-hooks a later fill skips
        self.check(solver)?;
        let pre = if fill_count == 0 {
            self.pre
                .iter()
                .map(|h| Hook::instruction(h, solver))
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };
        Ok(HookInstructions {
            pre,
            post: self
                .post
                .iter()
                .map(|h| Hook::instruction(h, solver))
                .collect::<Result<_, _>>()?,
        })
    }
}

/// The encoded instructions to add for hooks to a transaction
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookInstructions {
    pub pre: Vec<Instruction>,
    pub post: Vec<Instruction>,
}

impl Hook {
    fn check(&self, solver: &Pubkey) -> Result<(), InvalidHook> {
        // SECURITY: if a hook consumes the solver's account, it could use the
        // signature of the solver to drain their account or do other
        // nefarious things. Prevent this by excluding any hook which
        // references the solver's account.
        if self.program_id == *solver
            || self
                .accounts
                .iter()
                .any(|account| account.pubkey == *solver)
        {
            return Err(InvalidHook::ConsumesSolverSigner {
                program: self.program_id,
            });
        }
        Ok(())
    }

    /// The instruction the settlement transaction runs for this hook.
    pub fn instruction(&self, solver: &Pubkey) -> Result<Instruction, InvalidHook> {
        self.check(solver)?;
        Ok(Instruction {
            program_id: self.program_id,
            accounts: self
                .accounts
                .iter()
                .map(|account| AccountMeta {
                    pubkey: account.pubkey,
                    is_signer: false,
                    is_writable: account.is_writable,
                })
                .collect(),
            data: self.data.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn hook(program: u8, accounts: &[u8]) -> Hook {
        Hook {
            program_id: pubkey(program),
            accounts: accounts
                .iter()
                .map(|&byte| HookAccount {
                    pubkey: pubkey(byte),
                    is_writable: byte % 2 == 0,
                })
                .collect(),
            data: vec![0xde, 0xad],
        }
    }

    const SOLVER: u8 = 0xbb;

    #[test]
    fn accepts_hooks_away_from_the_solver() {
        let hooks = Hooks {
            pre: vec![hook(1, &[2, 3])],
            post: vec![hook(4, &[])],
        };
        assert_eq!(hooks.check(&pubkey(SOLVER)), Ok(()));
        assert!(!hooks.is_empty());
        assert!(Hooks::default().is_empty());
    }

    #[test]
    fn rejects_a_hook_consuming_the_solver_account() {
        for hooks in [
            Hooks {
                pre: vec![hook(1, &[2, SOLVER])],
                post: Vec::new(),
            },
            Hooks {
                pre: Vec::new(),
                post: vec![hook(1, &[]), hook(4, &[SOLVER])],
            },
            Hooks {
                pre: vec![hook(SOLVER, &[])],
                post: Vec::new(),
            },
        ] {
            assert!(matches!(
                hooks.check(&pubkey(SOLVER)),
                Err(InvalidHook::ConsumesSolverSigner { .. })
            ));
        }
    }

    #[test]
    fn instructions_sign_nothing() {
        let hooks = Hooks {
            pre: vec![hook(1, &[2, 3])],
            post: Vec::new(),
        };
        let instructions = hooks.instructions(&pubkey(SOLVER), 0).unwrap();
        assert_eq!(instructions.pre[0].program_id, pubkey(1));
        assert_eq!(instructions.pre[0].data, vec![0xde, 0xad]);
        assert_eq!(
            instructions.pre[0].accounts,
            vec![
                AccountMeta::new(pubkey(2), false),
                AccountMeta::new_readonly(pubkey(3), false),
            ]
        );
        assert!(instructions.post.is_empty());
    }

    /// A later fill runs the post-hooks only, and an invalid hook yields no
    /// instructions at all, even one this fill would skip.
    #[test]
    fn instructions_follow_the_fill_and_the_check() {
        let hooks = Hooks {
            pre: vec![hook(1, &[2])],
            post: vec![hook(4, &[])],
        };
        let later = hooks.instructions(&pubkey(SOLVER), 1).unwrap();
        assert!(later.pre.is_empty());
        assert_eq!(later.post.len(), 1);
        assert_eq!(later.post[0].program_id, pubkey(4));

        let invalid = Hooks {
            pre: vec![hook(1, &[SOLVER])],
            post: Vec::new(),
        };
        for fill_count in [0, 1, 2] {
            assert!(matches!(
                invalid.instructions(&pubkey(SOLVER), fill_count),
                Err(InvalidHook::ConsumesSolverSigner { .. })
            ));
        }
    }
}
