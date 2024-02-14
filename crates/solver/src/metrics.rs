/// The outcome of settlement submission.
#[derive(strum::EnumIter)]
pub enum SettlementSubmissionOutcome {
    /// A settlement transaction was mined and included on the blockchain.
    Success,
    /// A settlement transaction was mined and included on the blockchain but
    /// reverted.
    Revert,
    /// A transaction reverted in the simulation stage.
    SimulationRevert,
    /// Submission timed-out while waiting for the transaction to get mined.
    Timeout,
    /// Transaction sucessfully cancelled after simulation revert or timeout
    Cancel,
    /// Submission disabled
    Disabled,
    /// General message for failures (for example, failing to connect to client
    /// node)
    Failed,
}

impl SettlementSubmissionOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            SettlementSubmissionOutcome::Success => "success",
            SettlementSubmissionOutcome::Revert => "revert",
            SettlementSubmissionOutcome::Timeout => "timeout",
            SettlementSubmissionOutcome::Cancel => "cancel",
            SettlementSubmissionOutcome::SimulationRevert => "simulationrevert",
            SettlementSubmissionOutcome::Disabled => "disabled",
            SettlementSubmissionOutcome::Failed => "failed",
        }
    }
}
