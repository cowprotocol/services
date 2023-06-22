use crate::api::State;

pub(in crate::api) fn info(app: axum::Router<State>) -> axum::Router<State> {
    app.route("/", axum::routing::get(route))
}

async fn route() -> &'static str {
    "prices"
}
