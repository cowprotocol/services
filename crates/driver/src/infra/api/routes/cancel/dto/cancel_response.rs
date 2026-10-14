use {crate::domain::cancellation::Withdrawal, serde::Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelResponse {
    status: Status,
}

/// What the request found when it arrived. A solver that wants to know whether
/// its withdrawal was in time should still wait for the settlement
/// notification: `signalled` only says the submission path was told to stop,
/// not that it managed to before the transaction was mined.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Status {
    /// A settlement was being submitted and has been told to stop.
    Signalled,
    /// The solution had already been withdrawn by an earlier request.
    AlreadyWithdrawn,
    /// No settlement was being submitted for this solution. The withdrawal is
    /// remembered, so a submission that starts shortly after is withdrawn too.
    Recorded,
}

impl From<Withdrawal> for CancelResponse {
    fn from(withdrawal: Withdrawal) -> Self {
        let status = match withdrawal {
            Withdrawal::Signalled => Status::Signalled,
            Withdrawal::AlreadyWithdrawn => Status::AlreadyWithdrawn,
            Withdrawal::Recorded => Status::Recorded,
        };
        Self { status }
    }
}
