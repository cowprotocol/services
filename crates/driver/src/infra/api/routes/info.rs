use crate::infra::api::State;

pub(in crate::infra::api) fn info(app: axum::Router<State>) -> axum::Router<State> {
    app.route("/", axum::routing::get(route))
}

async fn route(state: axum::extract::State<State>) -> String {
    state.solver().name().to_string()
}
