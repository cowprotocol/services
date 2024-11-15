mod dto;

use {
    crate::{
        domain::competition,
        infra::{
            api::{Error, State},
            observe,
        },
    },
    tokio::sync::{mpsc, oneshot, OnceCell},
    tracing::Instrument,
};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle", axum::routing::post(route))
}

struct QueuedRequest {
    state: State,
    req: dto::Solution,
    response_sender: oneshot::Sender<Result<(), competition::Error>>,
}

static QUEUE_SENDER: OnceCell<mpsc::Sender<QueuedRequest>> = OnceCell::const_new();

async fn get_queue_sender() -> mpsc::Sender<QueuedRequest> {
    QUEUE_SENDER
        .get_or_init(|| async {
            let (sender, mut receiver) = mpsc::channel::<QueuedRequest>(100);

            // Spawn the background task to process the queue
            tokio::spawn(async move {
                while let Some(queued_request) = receiver.recv().await {
                    let QueuedRequest {
                        state,
                        req,
                        response_sender,
                    } = queued_request;

                    let auction_id = req.auction_id;
                    let solver = state.solver().name().to_string();

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
        })
        .await
        .clone()
}

async fn route(
    state: axum::extract::State<State>,
    req: axum::Json<dto::Solution>,
) -> Result<(), (hyper::StatusCode, axum::Json<Error>)> {
    let sender = get_queue_sender().await;
    let (response_tx, response_rx) = oneshot::channel();

    let queued_request = QueuedRequest {
        state: state.0.clone(),
        req: req.0,
        response_sender: response_tx,
    };

    sender.send(queued_request).await.map_err(|_| {
        <competition::Error as Into<(hyper::StatusCode, axum::Json<Error>)>>::into(
            competition::Error::UnableToEnqueue,
        )
    })?;

    match response_rx.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err(competition::Error::UnableToDequeue.into()),
    }
}
