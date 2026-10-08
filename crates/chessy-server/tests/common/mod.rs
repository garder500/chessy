//! Shared helpers: in-process clients, accounts and REST calls.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use chessy_engine::{parse_square, Action, Color, SkillId};
use chessy_server::hub::HubConfig;
use chessy_server::protocol::{ClientMsg, ServerMsg};
use chessy_server::store::Store;
use chessy_server::{router, App};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tower::ServiceExt;

pub struct Client {
    pub app: Arc<App>,
    rx: UnboundedReceiver<ServerMsg>,
    pub id: String,
    pub token: String,
    pub conn: u64,
    pub color: Option<Color>,
    pub welcome: Value,
    buffered: Vec<Value>,
}

impl Client {
    pub fn connect(app: &Arc<App>, token: Option<String>) -> Client {
        let (tx, rx) = unbounded_channel();
        let (id, conn) = app.connect(token, tx).unwrap();
        let mut c = Client {
            app: app.clone(),
            rx,
            id,
            token: String::new(),
            conn,
            color: None,
            welcome: Value::Null,
            buffered: Vec::new(),
        };
        c.welcome = c.next("welcome");
        c.token = c.welcome["token"].as_str().unwrap().to_string();
        c
    }

    pub fn send(&self, msg: ClientMsg) {
        self.app.handle(&self.id, self.conn, msg);
    }

    pub fn say(&self, v: Value) {
        self.send(serde_json::from_value(v).unwrap());
    }

    pub fn mv(&self, from: &str, to: &str) {
        self.send(ClientMsg::Action {
            action: Action::Move {
                from: parse_square(from).unwrap(),
                to: parse_square(to).unwrap(),
                promo: None,
            },
        });
    }

    pub fn pick_nothing(&self) {
        self.send(ClientMsg::SelectDeck { skills: vec![] });
    }

    pub fn pick(&self, skills: &[SkillId]) {
        self.send(ClientMsg::SelectDeck {
            skills: skills.to_vec(),
        });
    }

    fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.buffered.push(serde_json::to_value(&msg).unwrap());
        }
    }

    pub fn next(&mut self, ty: &str) -> Value {
        self.try_next(ty)
            .unwrap_or_else(|| panic!("no `{ty}` message arrived; have {:?}", self.types()))
    }

    pub fn try_next(&mut self, ty: &str) -> Option<Value> {
        self.drain();
        let at = self.buffered.iter().position(|v| v["type"] == ty)?;
        Some(self.buffered.remove(at))
    }

    /// The newest queued message of the given type; drains all of that type.
    pub fn last(&mut self, ty: &str) -> Value {
        let mut found = None;
        while let Some(v) = self.try_next(ty) {
            found = Some(v);
        }
        found.unwrap_or_else(|| panic!("no `{ty}` message arrived"))
    }

    pub fn types(&mut self) -> Vec<String> {
        self.drain();
        self.buffered
            .iter()
            .map(|v| v["type"].as_str().unwrap().to_string())
            .collect()
    }

    /// Drops everything received so far.
    pub fn clear(&mut self) {
        self.drain();
        self.buffered.clear();
    }

    pub async fn wait_for(&mut self, ty: &str) -> Value {
        for _ in 0..400 {
            if let Some(v) = self.try_next(ty) {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("timed out waiting for `{ty}`; have {:?}", self.types());
    }

    /// The next `notice` with this code (others stay queued).
    pub fn notice(&mut self, code: &str) -> Value {
        self.drain();
        let at = self
            .buffered
            .iter()
            .position(|v| v["type"] == "notice" && v["code"] == code)
            .unwrap_or_else(|| panic!("no `{code}` notice; have {:?}", self.buffered));
        self.buffered.remove(at)
    }

    pub fn has_notice(&mut self, code: &str) -> bool {
        self.drain();
        self.buffered
            .iter()
            .any(|v| v["type"] == "notice" && v["code"] == code)
    }

    pub fn error_code(&mut self) -> String {
        self.next("error")["code"].as_str().unwrap().to_string()
    }
}

pub fn new_app(config: HubConfig) -> (Arc<App>, Store) {
    let store = Store::open(":memory:").unwrap();
    (App::new(store.clone(), config), store)
}

/// An account (as if registered over REST) and a connected client for it.
pub fn account(app: &Arc<App>, store: &Store, name: &str) -> Client {
    let (_, token) = store.register(name, "unused-hash", None).unwrap();
    Client::connect(app, Some(token))
}

pub fn guest(app: &Arc<App>) -> Client {
    Client::connect(app, None)
}

/// Two clients matched (however they were queued): records colours and
/// picks no skills. Returns them in the order given, opening states consumed.
pub fn matched(mut a: Client, mut b: Client) -> (Client, Client) {
    a.color = serde_json::from_value(a.next("deck_select")["you"].clone()).ok();
    b.color = serde_json::from_value(b.next("deck_select")["you"].clone()).ok();
    assert_ne!(a.color, b.color);
    a.pick_nothing();
    b.pick_nothing();
    a.clear();
    b.clear();
    (a, b)
}

/// Like [`matched`], but returns `(white, black)`.
pub fn into_game(a: Client, b: Client) -> (Client, Client) {
    let (a, b) = matched(a, b);
    if a.color == Some(Color::White) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Two accounts matched in the ranked queue, in the order given.
pub fn ranked_match(a: Client, b: Client) -> (Client, Client) {
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    matched(a, b)
}

/// e4 e5 Nf3 Nc6 whichever way the colours fell: four plies, enough for a
/// game to count as rated.
pub fn four_plies(a: &Client, b: &Client) {
    let (white, black) = if a.color == Some(Color::White) {
        (a, b)
    } else {
        (b, a)
    };
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    white.mv("g1", "f3");
    black.mv("b8", "c6");
}

pub fn fools_mate(white: &Client, black: &Client) {
    white.mv("f2", "f3");
    black.mv("e7", "e5");
    white.mv("g2", "g4");
    black.mv("d8", "h4");
}

/// A database file that is removed on drop, for tests that need to reach
/// into the tables or start from an old schema.
pub struct TempDb {
    pub path: PathBuf,
}

impl TempDb {
    pub fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "chessy-test-{}-{}-{:08x}.sqlite",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
            rand::random::<u32>()
        ));
        TempDb { path }
    }

    pub fn path_str(&self) -> &str {
        self.path.to_str().unwrap()
    }

    pub fn raw(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.path).unwrap()
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Sets a player's rating state directly.
pub fn set_rating(db: &TempDb, player: &str, elo: i32, games: u32) {
    db.raw()
        .execute(
            "UPDATE players SET elo = ?2, peak_elo = MAX(peak_elo, ?2), games = ?3 WHERE id = ?1",
            rusqlite::params![player, elo, games],
        )
        .unwrap();
}

// ---- REST ---------------------------------------------------------------

pub struct Api {
    pub router: Router,
}

impl Api {
    pub fn new(app: &Arc<App>) -> Api {
        Api {
            router: router(app.clone()),
        }
    }

    pub async fn call(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let body = match body {
            Some(v) => {
                req = req.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = self
            .router
            .clone()
            .oneshot(req.body(body).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    pub async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.call("POST", uri, None, Some(body)).await
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.call("GET", uri, token, None).await
    }

    pub async fn register(&self, name: &str) -> String {
        let (status, v) = self
            .post(
                "/api/auth/register",
                json!({"username": name, "password": "correct horse"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{v}");
        v["token"].as_str().unwrap().to_string()
    }
}
