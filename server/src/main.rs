//! Cogneurs game server: accounts, asynchronous PvP and a leaderboard.
//!
//! Fights are computed here with the same engine as the browser, from the
//! fighters stored on the server, so a client cannot forge a result. The
//! browser only replays the events it receives.
//!
//! Storage is a single JSON file, rewritten after each change. That is
//! plenty for a prototype; a real database comes later.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use cogneurs_engine::combat::{fight, FightResult};
use cogneurs_engine::data::{Fighter, Rule, Stance};
use cogneurs_engine::progress;
use cogneurs_engine::rng::Rng;

mod tunnel;

// ---------- Storage ----------

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "k", rename_all = "snake_case")]
enum Offer {
    Player { name: String },
    Bot { fighter: Fighter },
}

#[derive(Serialize, Deserialize, Clone)]
struct Player {
    fighter: Fighter,
    offers: Vec<Offer>,
    #[serde(default)]
    day: u64,
    #[serde(default)]
    fights_today: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct FightLog {
    at: u64,
    attacker: String,
    defender: String,
    bot: bool,
    winner: u8,
}

#[derive(Serialize, Deserialize, Default)]
struct Db {
    /// Keyed by secret token.
    players: HashMap<String, Player>,
    fights: Vec<FightLog>,
}

struct App {
    db: Mutex<Db>,
    path: PathBuf,
    daily_fights: u32,
    rng: Mutex<Rng>,
}

impl App {
    async fn persist(&self, db: &Db) {
        let text = serde_json::to_string(db).expect("serializable");
        let tmp = self.path.with_extension("tmp");
        if let Some(dir) = self.path.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        if let Err(e) = tokio::fs::write(&tmp, text).await {
            eprintln!("save failed: {e}");
            return;
        }
        if let Err(e) = tokio::fs::rename(&tmp, &self.path).await {
            eprintln!("save failed: {e}");
        }
    }

    async fn seed(&self) -> u64 {
        self.rng.lock().await.next_u64()
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn random_u64() -> u64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("os randomness");
    u64::from_le_bytes(b)
}

fn new_token() -> String {
    let mut b = [0u8; 24];
    getrandom::fill(&mut b).expect("os randomness");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// ---------- Errors ----------

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "err": self.1 }))).into_response()
    }
}

fn bad(msg: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.to_string())
}

type ApiResult<T> = Result<Json<T>, ApiError>;

fn token(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "connexion requise".into()))
}

fn find_by_name<'a>(db: &'a Db, name: &str) -> Option<&'a Player> {
    db.players.values().find(|p| p.fighter.name.eq_ignore_ascii_case(name))
}

// ---------- Views ----------

#[derive(Serialize)]
struct OpponentView {
    fighter: Fighter,
    bot: bool,
}

#[derive(Serialize)]
struct HistoryView {
    at: u64,
    attacker: String,
    defender: String,
    won: bool,
    attacked: bool,
}

#[derive(Serialize)]
struct MeView {
    fighter: Fighter,
    options: Vec<progress::Upgrade>,
    opponents: Vec<OpponentView>,
    history: Vec<HistoryView>,
    fights_left: Option<u32>,
}

fn me_view(app: &App, db: &Db, p: &Player) -> MeView {
    let me = &p.fighter.name;
    let opponents = p
        .offers
        .iter()
        .filter_map(|o| match o {
            Offer::Bot { fighter } => Some(OpponentView { fighter: progress::public_view(fighter), bot: true }),
            Offer::Player { name } => {
                find_by_name(db, name).map(|q| OpponentView { fighter: progress::public_view(&q.fighter), bot: false })
            }
        })
        .collect();
    let history = db
        .fights
        .iter()
        .rev()
        .filter(|f| &f.attacker == me || &f.defender == me)
        .take(10)
        .map(|f| {
            let attacked = &f.attacker == me;
            HistoryView {
                at: f.at,
                attacker: f.attacker.clone(),
                defender: f.defender.clone(),
                won: (f.winner == 0) == attacked,
                attacked,
            }
        })
        .collect();
    let fights_left = (app.daily_fights > 0).then(|| {
        let used = if p.day == now() / 86400 { p.fights_today } else { 0 };
        app.daily_fights.saturating_sub(used)
    });
    MeView {
        fighter: p.fighter.clone(),
        options: if p.fighter.pending > 0 { progress::level_up_options(&p.fighter) } else { Vec::new() },
        opponents,
        history,
        fights_left,
    }
}

/// Three opponents: real players close in level first, bots to fill up.
fn make_offers(db: &Db, me: &Fighter, seed: u64) -> Vec<Offer> {
    let mut rng = Rng::new(seed);
    let mut pool: Vec<&Fighter> = db
        .players
        .values()
        .map(|p| &p.fighter)
        .filter(|f| f.name != me.name && f.level + 2 >= me.level && f.level <= me.level + 2)
        .collect();
    let mut offers = Vec::new();
    while offers.len() < 3 && !pool.is_empty() {
        let f = pool.swap_remove(rng.below(pool.len()));
        offers.push(Offer::Player { name: f.name.clone() });
    }
    let levels = [me.level.saturating_sub(1).max(1), me.level, me.level + 1];
    let mut k = 0;
    while offers.len() < 3 {
        let mut bot = progress::gen_opponent(levels[k % 3], rng.next_u64());
        bot.name.push_str(" (bot)");
        offers.push(Offer::Bot { fighter: bot });
        k += 1;
    }
    offers
}

// ---------- Handlers ----------

async fn health(State(app): State<Arc<App>>) -> Json<serde_json::Value> {
    let players = app.db.lock().await.players.len();
    Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION"), "players": players }))
}

#[derive(Deserialize)]
struct RegisterReq {
    name: String,
}

#[derive(Serialize)]
struct RegisterRes {
    token: String,
    me: MeView,
}

async fn register(State(app): State<Arc<App>>, Json(req): Json<RegisterReq>) -> ApiResult<RegisterRes> {
    let name = req.name.trim().to_string();
    let len = name.chars().count();
    if !(2..=16).contains(&len) {
        return Err(bad("Le nom doit faire entre 2 et 16 caractères."));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ') {
        return Err(bad("Lettres, chiffres, espaces, - et _ uniquement."));
    }
    if name.to_lowercase().contains("(bot)") {
        return Err(bad("Nom réservé."));
    }
    let seed = app.seed().await;
    let mut db = app.db.lock().await;
    if find_by_name(&db, &name).is_some() {
        return Err(ApiError(StatusCode::CONFLICT, "Ce nom est déjà pris.".into()));
    }
    let fighter = progress::new_fighter(&name, seed);
    let offers = make_offers(&db, &fighter, seed ^ 1);
    let token = new_token();
    db.players.insert(token.clone(), Player { fighter, offers, day: 0, fights_today: 0 });
    app.persist(&db).await;
    let me = me_view(&app, &db, &db.players[&token]);
    Ok(Json(RegisterRes { token, me }))
}

async fn get_me(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<MeView> {
    let t = token(&headers)?;
    let db = app.db.lock().await;
    let p = db.players.get(&t).ok_or_else(unknown)?;
    Ok(Json(me_view(&app, &db, p)))
}

fn unknown() -> ApiError {
    ApiError(StatusCode::UNAUTHORIZED, "Compte inconnu.".into())
}

#[derive(Deserialize)]
struct PlanReq {
    plan: Vec<Rule>,
    stance: Stance,
}

async fn put_plan(State(app): State<Arc<App>>, headers: HeaderMap, Json(mut req): Json<PlanReq>) -> ApiResult<MeView> {
    let t = token(&headers)?;
    progress::sanitize_plan(&mut req.plan);
    let mut db = app.db.lock().await;
    let p = db.players.get_mut(&t).ok_or_else(unknown)?;
    p.fighter.plan = req.plan;
    p.fighter.stance = req.stance;
    app.persist(&db).await;
    Ok(Json(me_view(&app, &db, &db.players[&t])))
}

#[derive(Deserialize)]
struct ChoiceReq {
    choice: usize,
}

async fn level_up(State(app): State<Arc<App>>, headers: HeaderMap, Json(req): Json<ChoiceReq>) -> ApiResult<MeView> {
    let t = token(&headers)?;
    let mut db = app.db.lock().await;
    let p = db.players.get_mut(&t).ok_or_else(unknown)?;
    if p.fighter.pending == 0 {
        return Err(bad("Aucune amélioration en attente."));
    }
    let opts = progress::level_up_options(&p.fighter);
    let u = *opts.get(req.choice).ok_or_else(|| bad("Choix invalide."))?;
    progress::apply(&mut p.fighter, u);
    app.persist(&db).await;
    Ok(Json(me_view(&app, &db, &db.players[&t])))
}

async fn reroll(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<MeView> {
    let t = token(&headers)?;
    let seed = app.seed().await;
    let mut db = app.db.lock().await;
    let me = db.players.get(&t).ok_or_else(unknown)?.fighter.clone();
    let offers = make_offers(&db, &me, seed);
    db.players.get_mut(&t).unwrap().offers = offers;
    app.persist(&db).await;
    Ok(Json(me_view(&app, &db, &db.players[&t])))
}

#[derive(Deserialize)]
struct FightReq {
    index: usize,
}

#[derive(Serialize)]
struct FightRes {
    result: FightResult,
    /// The opponent with its plan, revealed after the fight.
    foe: Fighter,
    me: MeView,
}

async fn post_fight(State(app): State<Arc<App>>, headers: HeaderMap, Json(req): Json<FightReq>) -> ApiResult<FightRes> {
    let t = token(&headers)?;
    let seed = app.seed().await;
    let mut db = app.db.lock().await;
    let p = db.players.get(&t).ok_or_else(unknown)?;
    if p.fighter.pending > 0 {
        return Err(bad("Choisis d'abord ton amélioration."));
    }
    let today = now() / 86400;
    let used = if p.day == today { p.fights_today } else { 0 };
    if app.daily_fights > 0 && used >= app.daily_fights {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "Plus de combats pour aujourd'hui, reviens demain.".into()));
    }
    let (foe, bot) = match p.offers.get(req.index).ok_or_else(|| bad("Adversaire inconnu."))? {
        Offer::Bot { fighter } => (fighter.clone(), true),
        Offer::Player { name } => (
            find_by_name(&db, name).ok_or_else(|| bad("Cet adversaire n'existe plus."))?.fighter.clone(),
            false,
        ),
    };
    let me = p.fighter.clone();
    let result = fight(&me, &foe);
    let won = result.winner == 0;

    let mut updated = me.clone();
    progress::record(&mut updated, won);
    let offers = make_offers(&db, &updated, seed);
    db.fights.push(FightLog { at: now(), attacker: me.name.clone(), defender: foe.name.clone(), bot, winner: result.winner });
    let p = db.players.get_mut(&t).unwrap();
    p.fighter = updated;
    p.offers = offers;
    p.day = today;
    p.fights_today = used + 1;
    app.persist(&db).await;
    let me = me_view(&app, &db, &db.players[&t]);
    Ok(Json(FightRes { result, foe, me }))
}

#[derive(Serialize)]
struct Rank {
    name: String,
    level: u32,
    wins: u32,
    losses: u32,
}

async fn leaderboard(State(app): State<Arc<App>>) -> Json<Vec<Rank>> {
    let db = app.db.lock().await;
    let mut all: Vec<Rank> = db
        .players
        .values()
        .map(|p| Rank { name: p.fighter.name.clone(), level: p.fighter.level, wins: p.fighter.wins, losses: p.fighter.losses })
        .collect();
    all.sort_by(|a, b| (b.level, b.wins).cmp(&(a.level, a.wins)).then(a.losses.cmp(&b.losses)));
    all.truncate(20);
    Json(all)
}

// ---------- Main ----------

#[tokio::main]
async fn main() {
    // Paths default to the project folder: the current directory if it holds
    // `web/`, else two levels above the executable (target/release).
    let root = if std::path::Path::new("web").is_dir() {
        PathBuf::from(".")
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.ancestors().nth(3).map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let path = std::env::var("COGNEURS_DATA").map(PathBuf::from).unwrap_or_else(|_| root.join("data/cogneurs.json"));
    let web = std::env::var("COGNEURS_WEB").map(PathBuf::from).unwrap_or_else(|_| root.join("web"));
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8137);
    let daily_fights: u32 = std::env::var("COGNEURS_DAILY_FIGHTS").ok().and_then(|p| p.parse().ok()).unwrap_or(20);

    let db: Db = match tokio::fs::read_to_string(&path).await {
        Ok(text) => serde_json::from_str(&text).expect("invalid save file"),
        Err(_) => Db::default(),
    };
    println!("{} players loaded from {}", db.players.len(), path.display());

    let app = Arc::new(App { db: Mutex::new(db), path, daily_fights, rng: Mutex::new(Rng::new(random_u64())) });

    let api = Router::new()
        .route("/health", get(health))
        .route("/register", post(register))
        .route("/me", get(get_me))
        .route("/me/plan", put(put_plan))
        .route("/me/level", post(level_up))
        .route("/me/reroll", post(reroll))
        .route("/fight", post(post_fight))
        .route("/leaderboard", get(leaderboard))
        .with_state(app);

    let router = Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(web))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port in use");
    println!("Cogneurs on http://localhost:{port}");

    // --tunnel [--gist ID]: home hosting through a Cloudflare quick tunnel.
    let args: Vec<String> = std::env::args().collect();
    let gist = args.iter().position(|a| a == "--gist").and_then(|i| args.get(i + 1)).cloned();
    let mut tunnel = None;
    if args.iter().any(|a| a == "--tunnel") {
        println!("Ouverture du tunnel...");
        match tunnel::start(port, gist).await {
            Ok(t) => {
                println!();
                println!("  EN LIGNE : {}", t.url);
                println!("  La page https://tom-tour.github.io/cogneurs/ se connecte toute seule.");
                println!("  Ferme cette fenêtre (ou Ctrl+C) pour mettre le serveur hors ligne.");
                println!();
                tunnel = Some(t);
            }
            Err(e) => eprintln!("Tunnel impossible : {e}\nLe serveur reste accessible en local seulement."),
        }
    }

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            println!("Arrêt...");
            if let Some(t) = tunnel {
                t.stop().await;
            }
        })
        .await
        .unwrap();
}

/// Ctrl+C, or closing the console window on Windows.
async fn shutdown_signal() {
    #[cfg(windows)]
    {
        let mut close = tokio::signal::windows::ctrl_close().expect("console signal");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = close.recv() => {}
        }
    }
    #[cfg(not(windows))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
