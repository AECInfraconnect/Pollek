use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub async fn sse_handler(
    State(state): State<crate::state::AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.bundle_tx.subscribe();
    let mut broadcast_stream = BroadcastStream::new(receiver);
    let mut keepalive_interval = interval(Duration::from_secs(15));

    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                _ = keepalive_interval.tick() => {
                    yield Ok(Event::default().event("keepalive").data("ping"));
                }
                msg = broadcast_stream.next() => {
                    match msg {
                        Some(Ok(version)) => {
                            yield Ok(Event::default().event("bundle_ready").data(version));
                        }
                        Some(Err(_)) => {
                            // lagging
                        }
                        None => break, // closed
                    }
                }
            }
        }
    };

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().text("keepalive"))
}
