use crate::infra::api::{Error, State};

pub(in crate::infra::api) fn settle(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/settle/:solution_id", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    axum::extract::Path(solution_id): axum::extract::Path<u64>,
) -> Result<(), axum::Json<Error>> {
    let competition = state.competition();
    competition
        .settle(solution_id.into())
        .await
        .map_err(Into::into)
}
