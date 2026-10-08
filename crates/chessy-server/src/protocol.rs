//! WebSocket messages, JSON-encoded as `{"type": "...", ...}`.
//! Squares are indices `0..64` with `a1 = 0` and `h8 = 63`.

use chessy_engine::{
    Action, ActiveEffect, Color, Event, Move, Outcome, Piece, PieceKind, SkillId, SkillTarget,
    Square,
};
use serde::{Deserialize, Serialize};

use crate::hub::SpectatorView;

pub type PlayerId = String;

/// Game length a player asks for. Absent means the server's default clock (`HubConfig::clock_initial`).
/// Players only meet an opponent who asked for the same length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeControl {
    /// Blitz, 5 minutes.
    Short,
    /// 15 minutes.
    Medium,
    /// 30 minutes.
    Long,
}

impl TimeControl {
    /// Time on each clock at the start of the game.
    pub fn initial(self) -> std::time::Duration {
        std::time::Duration::from_secs(match self {
            TimeControl::Short => 5 * 60,
            TimeControl::Medium => 15 * 60,
            TimeControl::Long => 30 * 60,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Must be the first message. Without a known token a new player is created.
    Hello {
        token: Option<String>,
    },
    /// `ranked` defaults to true for accounts and is forced to false for guests.
    QueueJoin {
        #[serde(default)]
        ranked: Option<bool>,
        #[serde(default)]
        time: Option<TimeControl>,
    },
    CreateRoom {
        #[serde(default)]
        time: Option<TimeControl>,
    },
    JoinRoom {
        code: String,
    },
    LeaveLobby,
    /// Walks out of deck selection (back to the lobby). Against the bot the game is dropped;
    /// against a queue opponent they go back to waiting in the queue; otherwise the game is cancelled.
    LeaveDeckSelect,
    /// Classic skills to bring (at most 3); unique skills in the deck come along for free.
    SelectDeck {
        skills: Vec<SkillId>,
    },
    Action {
        action: Action,
    },
    Resign,
    RewardChoice {
        choice: RewardChoice,
    },
    OfferDraw,
    RespondDraw {
        accept: bool,
    },
    Chat {
        text: String,
    },
    RematchRequest,
    RematchRespond {
        accept: bool,
    },
    // Social messages: accounts only.
    FriendRequest {
        username: String,
    },
    FriendRespond {
        username: String,
        accept: bool,
    },
    FriendRemove {
        username: String,
    },
    FriendsList,
    // Moderation (accounts only): see docs/spec-v2.md §4, "Modération".
    BlockUser {
        username: String,
    },
    UnblockUser {
        username: String,
    },
    BlocksList,
    /// Drops (or again accepts) every incoming chat message of the account.
    SetChatMuted {
        muted: bool,
    },
    /// `game_id` and `context` (a chat excerpt, at most 1 KiB) are optional.
    ReportUser {
        username: String,
        reason: ReportReason,
        #[serde(default)]
        game_id: Option<String>,
        #[serde(default)]
        context: Option<String>,
    },
    UserSearch {
        query: String,
    },
    Challenge {
        username: String,
        #[serde(default)]
        time: Option<TimeControl>,
    },
    ChallengeRespond {
        username: String,
        accept: bool,
    },
    ChallengeCancel,
    /// Starts a friendly game against the bot. `elo` is 400..=2800.
    SoloStart {
        elo: i64,
        #[serde(default)]
        color: SoloColor,
    },
    /// Watches a running game (not allowed while playing).
    Spectate {
        game_id: String,
    },
    Unspectate,
}

/// Why a player is reported: a closed set, so a report holds no free text
/// other than the optional chat excerpt.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportReason {
    Spam,
    Harassment,
    Cheating,
    InappropriateName,
    Other,
}

impl ReportReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ReportReason::Spam => "spam",
            ReportReason::Harassment => "harassment",
            ReportReason::Cheating => "cheating",
            ReportReason::InappropriateName => "inappropriate_name",
            ReportReason::Other => "other",
        }
    }
}

/// Which side the human takes in a solo game.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoloColor {
    White,
    Black,
    #[default]
    Random,
}

/// What the winner takes. `replace` names the skill to drop when the deck is full.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RewardChoice {
    /// Take a specific skill from the loser's deck.
    Steal {
        skill: SkillId,
        replace: Option<SkillId>,
    },
    /// Roll a random skill from the global pool; the loser loses a random one.
    Random {
        replace: Option<SkillId>,
    },
    Skip,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyStatus {
    Idle,
    Queued { ranked: bool },
    RoomWaiting { code: String },
}

/// The signed-in player (`GET /api/me`, `welcome.account`).
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Me {
    pub player_id: PlayerId,
    pub username: Option<String>,
    pub guest: bool,
    pub elo: i32,
    /// Rank among registered accounts; `None` for guests.
    pub rank: Option<u32>,
    pub games: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    /// The account drops every incoming chat message (`set_chat_muted`).
    pub chat_muted: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OpponentInfo {
    pub username: Option<String>,
    pub elo: Option<i32>,
    pub guest: bool,
    /// The opponent is the Solo bot. Omitted (false) for people.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bot: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ClockView {
    pub white_ms: u64,
    pub black_ms: u64,
    pub running: Option<Color>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DrawOffer {
    None,
    You,
    Them,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct EloView {
    pub you_before: i32,
    pub you_after: i32,
    pub opp_before: i32,
    pub opp_after: i32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Online,
    InGame,
    Offline,
}

#[derive(Clone, Debug, Serialize)]
pub struct FriendInfo {
    pub username: String,
    pub elo: i32,
    pub presence: Presence,
    pub last_seen: Option<String>,
    /// The game they are playing right now, if it can be watched.
    pub game_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UserRef {
    pub username: String,
    pub elo: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct NameRef {
    pub username: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    None,
    Friend,
    Incoming,
    Outgoing,
    #[serde(rename = "self")]
    SelfUser,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResult {
    pub username: String,
    pub elo: i32,
    pub relation: Relation,
}

#[derive(Clone, Debug, Serialize)]
pub struct RewardOffer {
    pub deck: Vec<SkillId>,
    pub steal_options: Vec<SkillId>,
    pub deck_full: bool,
}

/// A skill and everywhere it can currently be aimed.
#[derive(Clone, Debug, Serialize)]
pub struct SkillOptions {
    pub skill: SkillId,
    pub targets: Vec<SkillTarget>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OpponentSkills {
    pub total: usize,
    pub used: Vec<SkillId>,
}

/// One of your skills with how often it has been used.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SkillSlotView {
    pub skill: SkillId,
    /// True once `uses` reached `max_uses`.
    pub used: bool,
    pub uses: u8,
    pub max_uses: u8,
}

/// Geomancy terrain: `owner`'s pieces cross it freely, the others cannot.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct TerrainView {
    pub square: Square,
    pub owner: Color,
    pub expires_at: u32,
}

/// A piece that ended an action on `square`: lets a client name what moved
/// without the board of that time.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct Landed {
    pub square: Square,
    pub kind: PieceKind,
}

/// One past action as its receiver saw it (events filtered like the live
/// ones). Sent with the state a player gets on (re)connection, so the journal
/// of a game survives a page reload.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// The ply of the position the action led to.
    pub ply: u32,
    /// Who was to move after the action.
    pub to_move: Color,
    pub events: Vec<Event>,
    /// Where the pieces of the `Moved` events stand now.
    pub landed: Vec<Landed>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StateView {
    pub game_id: String,
    pub you: Color,
    pub ply: u32,
    pub to_move: Color,
    pub in_check: bool,
    pub board: Vec<Option<Piece>>,
    /// Empty unless it is your turn.
    pub moves: Vec<Move>,
    pub skill_options: Vec<SkillOptions>,
    pub my_skills: Vec<SkillSlotView>,
    pub opponent_skills: OpponentSkills,
    /// Effects on pieces (terrain is in `terrain`); effects on pieces hidden
    /// from you are left out.
    pub effects: Vec<ActiveEffect>,
    /// Your own traps (the opponent's are secret).
    pub traps: Vec<Square>,
    /// Your own pieces currently on the bench.
    pub benched: Vec<Piece>,
    pub terrain: Vec<TerrainView>,
    pub outcome: Outcome,
    /// What the last action did, for animation. Empty on resume.
    pub events: Vec<Event>,
    pub opponent_connected: bool,
    pub clock: ClockView,
    /// False when the game has no clock (Solo): `clock.running` is then null.
    pub clock_enabled: bool,
    pub rated: bool,
    pub opponent: OpponentInfo,
    pub draw_offer: DrawOffer,
    pub ply_count: u32,
    /// People watching the game right now.
    pub spectators: usize,
    /// Every action played so far, oldest first. Only sent when a player
    /// (re)joins their game: live states carry just `events`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        player_id: PlayerId,
        token: String,
        deck: Vec<SkillId>,
        pending_reward: Option<RewardOffer>,
        account: Me,
    },
    Lobby {
        status: LobbyStatus,
    },
    DeckSelect {
        game_id: String,
        you: Color,
        deck: Vec<SkillId>,
        max_picks: usize,
        seconds: u64,
        submitted: bool,
        opponent: OpponentInfo,
        rated: bool,
    },
    State(Box<StateView>),
    OpponentStatus {
        connected: bool,
    },
    GameOver {
        outcome: Outcome,
        reward: Option<RewardOffer>,
        rated: bool,
        elo: Option<EloView>,
        reason: String,
    },
    /// Your deck changed (reward applied, or you lost a skill).
    DeckUpdate {
        deck: Vec<SkillId>,
        gained: Option<SkillId>,
        lost: Option<SkillId>,
    },
    GameCancelled {
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
    Friends {
        friends: Vec<FriendInfo>,
        incoming: Vec<UserRef>,
        outgoing: Vec<NameRef>,
    },
    UserResults {
        query: String,
        users: Vec<SearchResult>,
    },
    /// The accounts the player blocked, after `blocks_list`, `block_user` and `unblock_user`.
    Blocks {
        blocked: Vec<NameRef>,
    },
    /// The chat mute setting, after `set_chat_muted`.
    ChatSettings {
        chat_muted: bool,
    },
    /// A report was received (also when it duplicated a recent one).
    ReportAck {
        username: String,
    },
    /// A short event for a toast; `username` names the other party when relevant.
    Notice {
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        username: Option<String>,
    },
    ChallengeReceived {
        from: UserRef,
    },
    ChallengeSent {
        username: String,
    },
    DrawOffered {},
    DrawDeclined {},
    Chat {
        text: String,
        mine: bool,
    },
    RematchOffered {},
    RematchDeclined {},
    /// A spectator's picture of the game: on entering, then after every action.
    SpectateState {
        view: Box<SpectatorView>,
    },
    /// The game watched has ended; the spectator is released.
    SpectateOver {
        view: Box<SpectatorView>,
    },
    /// The game watched was cancelled.
    SpectateEnded {
        reason: String,
    },
}

/// Error codes after which the connection is closed.
pub const FATAL_ERRORS: [&str; 2] = ["session_revoked", "flooded"];

impl ClientMsg {
    /// What the message costs against the per-connection quota: the ones that
    /// query the database or notify other people weigh more.
    pub fn cost(&self, expensive: u32) -> u32 {
        match self {
            ClientMsg::UserSearch { .. }
            | ClientMsg::FriendRequest { .. }
            | ClientMsg::FriendRespond { .. }
            | ClientMsg::FriendRemove { .. }
            | ClientMsg::FriendsList
            | ClientMsg::BlockUser { .. }
            | ClientMsg::UnblockUser { .. }
            | ClientMsg::BlocksList
            | ClientMsg::SetChatMuted { .. }
            | ClientMsg::ReportUser { .. }
            | ClientMsg::Challenge { .. }
            | ClientMsg::ChallengeRespond { .. }
            | ClientMsg::SoloStart { .. } => expensive,
            _ => 1,
        }
    }
}

impl ServerMsg {
    /// Whether the server closes the connection right after sending this.
    pub fn closes_connection(&self) -> bool {
        matches!(self, ServerMsg::Error { code, .. } if FATAL_ERRORS.contains(&code.as_str()))
    }

    pub fn notice(code: &str, username: Option<&str>) -> Self {
        ServerMsg::Notice {
            code: code.to_string(),
            // Often an echo of what the client typed: keep it short.
            username: username.map(|u| u.chars().take(32).collect()),
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        ServerMsg::Error {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}
