//! Caps on simultaneous WebSocket connections, over real sockets.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use chessy_server::hub::HubConfig;
use chessy_server::store::Store;
use chessy_server::{router, App};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A server bound to an ephemeral port. With `connect_info` the handler sees
/// the peer address (as `main` does); without it, as the other test files
/// serve, only the global cap can apply.
async fn spawn_with(config: HubConfig, connect_info: bool) -> (String, Arc<App>) {
    let store = Store::open(":memory:").unwrap();
    let app = App::new(store, config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let routes = router(app.clone());
    tokio::spawn(async move {
        if connect_info {
            axum::serve(
                listener,
                routes.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap()
        } else {
            axum::serve(listener, routes).await.unwrap()
        }
    });
    (format!("ws://{addr}/ws"), app)
}

fn capped(max_connections: usize, per_ip: usize, trust_proxy: bool) -> HubConfig {
    HubConfig {
        max_connections,
        max_connections_per_ip: per_ip,
        trust_proxy,
        ..HubConfig::default()
    }
}

async fn open(url: &str) -> Result<Socket, Error> {
    tokio_tungstenite::connect_async(url)
        .await
        .map(|(ws, _)| ws)
}

/// Opens a socket as if a proxy had forwarded it for `forwarded_for`.
async fn open_via_proxy(url: &str, forwarded_for: &str) -> Result<Socket, Error> {
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("x-forwarded-for", forwarded_for.parse().unwrap());
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(ws, _)| ws)
}

/// The status of a refused upgrade; panics if the socket opened.
fn refused(result: Result<Socket, Error>) -> StatusCode {
    match result {
        Err(Error::Http(response)) => {
            assert_eq!(response.headers()["retry-after"], "10");
            response.status()
        }
        Err(other) => panic!("expected an HTTP refusal, got {other:?}"),
        Ok(_) => panic!("expected a refusal, but the socket opened"),
    }
}

/// Slots come back once the server notices the socket is gone, which is not
/// instantaneous: wait for the open count to reach `n`.
async fn settle(app: &Arc<App>, n: usize) {
    for _ in 0..250 {
        if app.open_connections() == n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!(
        "expected {n} open connections, have {}",
        app.open_connections()
    );
}

async fn hello(ws: &mut Socket) {
    ws.send(Message::Text(json!({"type": "hello"}).to_string().into()))
        .await
        .unwrap();
    loop {
        let Message::Text(t) = ws.next().await.expect("socket open").unwrap() else {
            continue;
        };
        if t.contains("\"welcome\"") {
            return;
        }
    }
}

#[tokio::test]
async fn the_global_cap_refuses_one_more_and_accepts_again_after_a_close() {
    let (url, app) = spawn_with(capped(3, 0, false), true).await;
    let mut sockets = Vec::new();
    for _ in 0..3 {
        sockets.push(open(&url).await.unwrap());
    }
    assert_eq!(app.open_connections(), 3);
    assert_eq!(refused(open(&url).await), StatusCode::SERVICE_UNAVAILABLE);
    // The refusal took no slot.
    assert_eq!(app.open_connections(), 3);

    let mut gone = sockets.pop().unwrap();
    gone.close(None).await.unwrap();
    settle(&app, 2).await;
    let mut again = open(&url).await.expect("a place was freed");
    hello(&mut again).await;
    assert_eq!(app.open_connections(), 3);
    assert_eq!(refused(open(&url).await), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn the_cap_holds_without_connect_info_too() {
    let (url, app) = spawn_with(capped(2, 1, false), false).await;
    let _a = open(&url).await.unwrap();
    // No peer address: the per-IP cap cannot apply, the global one does.
    let _b = open(&url).await.unwrap();
    assert_eq!(refused(open(&url).await), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(app.open_connections(), 2);
}

#[tokio::test]
async fn the_per_ip_cap_counts_the_peer_address() {
    let (url, app) = spawn_with(capped(0, 2, false), true).await;
    let _a = open(&url).await.unwrap();
    let mut b = open(&url).await.unwrap();
    assert_eq!(refused(open(&url).await), StatusCode::SERVICE_UNAVAILABLE);
    b.close(None).await.unwrap();
    settle(&app, 1).await;
    assert!(open(&url).await.is_ok());
}

#[tokio::test]
async fn an_abrupt_drop_gives_the_slot_back() {
    let (url, app) = spawn_with(capped(1, 1, false), true).await;
    let mut ws = open(&url).await.unwrap();
    hello(&mut ws).await;
    assert_eq!(refused(open(&url).await), StatusCode::SERVICE_UNAVAILABLE);
    // No close frame: the TCP stream is simply dropped.
    drop(ws);
    settle(&app, 0).await;
    assert!(open(&url).await.is_ok());
}

#[tokio::test]
async fn a_flood_disconnect_gives_the_slot_back() {
    let config = HubConfig {
        msg_rate: 0.1,
        msg_burst: 1,
        flood_disconnect_after: 3,
        ..capped(1, 0, false)
    };
    let (url, app) = spawn_with(config, true).await;
    let mut ws = open(&url).await.unwrap();
    for _ in 0..20 {
        if ws
            .send(Message::Text("{\"type\":\"resign\"}".into()))
            .await
            .is_err()
        {
            break;
        }
    }
    // The server hangs up on its own.
    let closed = async {
        let mut flooded = false;
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return flooded,
                Some(Ok(Message::Text(t))) => flooded |= t.contains("\"flooded\""),
                Some(Ok(_)) => continue,
            }
        }
    };
    let flooded = tokio::time::timeout(Duration::from_secs(5), closed)
        .await
        .expect("the flooder was disconnected");
    assert!(flooded, "the flood error came before the close");
    settle(&app, 0).await;
    assert!(open(&url).await.is_ok());
}

#[tokio::test]
async fn the_default_config_does_not_get_in_the_way() {
    let (url, app) = spawn_with(HubConfig::default(), true).await;
    assert_eq!(HubConfig::default().max_connections_per_ip, 0);
    assert!(!HubConfig::default().trust_proxy);
    let mut sockets = Vec::new();
    for _ in 0..40 {
        sockets.push(open(&url).await.unwrap());
    }
    for ws in sockets.iter_mut().take(3) {
        hello(ws).await;
    }
    assert_eq!(app.open_connections(), 40);
}

#[tokio::test]
async fn the_forwarded_header_is_ignored_unless_the_proxy_is_trusted() {
    // Every client below is 127.0.0.1 for the server; the header lies.
    let (url, _app) = spawn_with(capped(0, 1, false), true).await;
    let _a = open_via_proxy(&url, "198.51.100.1").await.unwrap();
    assert_eq!(
        refused(open_via_proxy(&url, "198.51.100.2").await),
        StatusCode::SERVICE_UNAVAILABLE,
        "a client picking its own address must not dodge the cap"
    );
}

#[tokio::test]
async fn a_trusted_proxy_header_splits_users_by_their_forwarded_address() {
    let (url, app) = spawn_with(capped(0, 1, true), true).await;
    let _a = open_via_proxy(&url, "198.51.100.1").await.unwrap();
    let _b = open_via_proxy(&url, "198.51.100.2").await.unwrap();
    assert_eq!(
        refused(open_via_proxy(&url, "198.51.100.1").await),
        StatusCode::SERVICE_UNAVAILABLE
    );
    // Only the right-most entry (the proxy's) counts: spoofed lefts change nothing.
    assert_eq!(
        refused(open_via_proxy(&url, "192.0.2.77, 198.51.100.1").await),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let _c = open_via_proxy(&url, "192.0.2.77, 198.51.100.3")
        .await
        .unwrap();
    assert_eq!(app.open_connections(), 3);
    // Without the header, the peer address is used.
    let _d = open(&url).await.unwrap();
    assert_eq!(refused(open(&url).await), StatusCode::SERVICE_UNAVAILABLE);
}
