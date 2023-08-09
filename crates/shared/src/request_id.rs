//! This module supplies the tools to associate 1 identifier with a task.
//! That identifier is accessable globally just for that task. The idea
//! is that this identifier is supposed to tie together related logs. That
//! is easy to accomplish in a single process (simply use a tracing span)
//! but if you want to tie together logs across multiple processes things
//! can get messier.
//! The most obvious option is to take that identifier and pass that through
//! the process until you make some request to another process and give that
//! process the identifier in your request.
//! However, if would do that the identifier would basically show up everywhere
//! although other components don't care about it and it doesn't even change
//! any behaviour in the process.
//! Instead we use task local storage that is globally visible but only
//! individual to each task. That way we can populate the storage with the
//! identifier once and not care about dragging it through the code base.
//! And when we issue requests to another process we can simply fetch the
//! current identifier specific to our task and send that along with the
//! request.
tokio::task_local! {
    pub static REQUEST_ID: std::cell::RefCell<String>;
}

/// Reads the `request_id` from this tasks storage.
/// Returns `None` if task local storage was not initialized or is empty.
pub fn get_task_local_storage() -> Option<String> {
    let mut id = None;
    let _ = REQUEST_ID.try_with(|cell| {
        id = Some(cell.borrow().clone());
    });
    id
}

/// Writes the `request_id` to the task local storage.
/// Panics if called in a task that was not created with `REQUEST_ID.sync()`.
pub fn set_task_local_storage(request_id: String) {
    REQUEST_ID.with(|storage| {
        *storage.borrow_mut() = request_id;
    });
}

/// Takes a `tower::Service` and embeds it in a `make_service` function that
/// spawns one of these services per incoming request.
/// But crucially before spawning that service some task local storage will
/// also be initialized.
#[macro_export]
macro_rules! make_service_with_task_local_storage {
    ($service:expr) => {{
        hyper::service::make_service_fn(move |_| {
            let warp_svc = $service.clone();
            async move {
                let svc = hyper::service::service_fn(move |req: hyper::Request<hyper::Body>| {
                    let mut warp_svc = warp_svc.clone();
                    shared::request_id::REQUEST_ID.scope(Default::default(), async move {
                        // Not sure why but we have to have this async block to avoid panics
                        hyper::service::Service::call(&mut warp_svc, req).await
                    })
                });
                Ok::<_, std::convert::Infallible>(svc)
            }
        })
    }};
}
