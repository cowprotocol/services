mod dto;

use {
    crate::{
        domain::competition,
        infra::{
            api::{Error, State},
            observe,
        },
    },
    tokio::sync::{mpsc, oneshot},
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

pub(in crate::infra::api) struct QueuedSettleRequest {
    state: State,
    req: dto::Solution,
    response_sender: oneshot::Sender<Result<(), competition::Error>>,
}

pub(in crate::infra::api) fn create_settle_queue_sender(
    queue_size: usize,
) -> mpsc::Sender<QueuedSettleRequest> {
    let (sender, mut receiver) = mpsc::channel::<QueuedSettleRequest>(queue_size);

    // Spawn the background task to process the queue
    tokio::spawn(async move {
        while let Some(queued_request) = receiver.recv().await {
            let QueuedSettleRequest {
                state,
                req,
                response_sender,
            } = queued_request;

            let auction_id = req.auction_id;
            let solver = state.solver().name().to_string();
            if state.eth().current_block().borrow().number >= req.submission_deadline_latest_block {
                if let Err(err) =
                    response_sender.send(Err(competition::Error::QueueAwaitingDeadlineExceeded))
                {
                    tracing::error!(
                        ?err,
                        "settle deadline exceeded. unable to return a response"
                    );
                }
                return;
            }

            let result = async move {
                observe::settling();
                let result = state
                    .competition()
                    .settle(
                        req.auction_id,
                        req.solution_id,
                        req.submission_deadline_latest_block,
                    )
                    .await;
                observe::settled(state.solver().name(), &result);
                result.map(|_| ()).map_err(Into::into)
            }
            .instrument(tracing::info_span!("/settle", solver, auction_id))
            .await;

            if let Err(err) = response_sender.send(result) {
                tracing::error!(?err, "Failed to send /settle response");
            }
        }
    });

    sender
}

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::Solution>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let sender = state.settle_queue_sender();
    let (response_tx, response_rx) = oneshot::channel();

    let queued_request = QueuedSettleRequest {
        state: state.0.clone(),
        req: req.0,
        response_sender: response_tx,
    };

    sender.send(queued_request).await.map_err(|err| {
        tracing::error!(?err, "Failed to enqueue /settle request");
        <competition::Error as Into<(hyper::StatusCode, axum::Json<Error>)>>::into(
            competition::Error::UnableToEnqueue,
        )
    })?;

    match response_rx.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.into()),
        Err(err) => {
            tracing::error!(?err, "Failed to dequeue /settle response");
            Err(competition::Error::UnableToDequeue.into())
        }
    }
}
