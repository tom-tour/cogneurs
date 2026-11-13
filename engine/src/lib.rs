//! Cogneurs game engine, shared by the browser (WebAssembly) and the server.
//! In the browser it exposes a single JSON entry point: the page writes a
//! request into wasm memory, calls `call`, then reads the JSON answer back.

pub mod combat;
pub mod data;
pub mod progress;
pub mod rng;

use serde::Deserialize;
use serde_json::{json, Value};

use data::*;

#[derive(Deserialize)]
#[serde(tag = "fn", content = "args", rename_all = "snake_case")]
enum Call {
    NewFighter { name: String },
    Info { fighter: Fighter },
    SetBuild { fighter: Fighter, stats: Stats, picks: Vec<Pick> },
    Record { fighter: Fighter, won: bool },
    Opponents { fighter: Fighter, seed: u64 },
    Fight { a: Fighter, b: Fighter },
    Predict { a: Fighter, b: Fighter, n: u32, seed: u64 },
    /// Loads a saved fighter (converting older saves) and returns it.
    Load { fighter: Fighter },
}

pub fn handle(request: &str) -> String {
    let call: Call = match serde_json::from_str(request) {
        Ok(c) => c,
        Err(e) => return json!({ "err": e.to_string() }).to_string(),
    };
    let ok: Value = match call {
        Call::NewFighter { name } => json!(progress::new_fighter(&name)),
        Call::Info { fighter } => json!(progress::build_info(&fighter)),
        Call::SetBuild { mut fighter, stats, picks } => match progress::set_build(&mut fighter, stats, picks) {
            Ok(()) => json!(fighter),
            Err(e) => return json!({ "err": e }).to_string(),
        },
        Call::Record { mut fighter, won } => {
            progress::record(&mut fighter, won);
            json!(fighter)
        }
        Call::Opponents { fighter, seed } => json!(progress::gen_opponents(&fighter, seed)),
        Call::Fight { a, b } => json!(combat::fight(&a, &b)),
        Call::Predict { a, b, n, seed } => json!(progress::predict(&a, &b, n.min(500), seed)),
        Call::Load { fighter } => json!(fighter),
    };
    json!({ "ok": ok }).to_string()
}

// ---- WebAssembly interface -------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod ffi {
    use std::cell::RefCell;

    thread_local! {
        static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    #[no_mangle]
    pub extern "C" fn alloc(len: usize) -> *mut u8 {
        let mut v = Vec::<u8>::with_capacity(len);
        let p = v.as_mut_ptr();
        std::mem::forget(v);
        p
    }

    /// # Safety
    /// `ptr` must come from `alloc(cap)`.
    #[no_mangle]
    pub unsafe extern "C" fn dealloc(ptr: *mut u8, cap: usize) {
        drop(Vec::from_raw_parts(ptr, 0, cap));
    }

    /// # Safety
    /// `ptr..ptr+len` must be a UTF-8 buffer obtained from `alloc`.
    #[no_mangle]
    pub unsafe extern "C" fn call(ptr: *const u8, len: usize) -> *const u8 {
        let req = std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("");
        let out = super::handle(req).into_bytes();
        OUT.with(|o| {
            *o.borrow_mut() = out;
            o.borrow().as_ptr()
        })
    }

    #[no_mangle]
    pub extern "C" fn out_len() -> usize {
        OUT.with(|o| o.borrow().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat::fight;
    use progress::*;

    /// Level 4 sword fighter with a potion, every point spent.
    fn armed(name: &str) -> Fighter {
        let mut f = new_fighter(name);
        f.level = 4;
        let stats = Stats { str: 2, tou: 2, acc: 1, eva: 1, spd: 2, end: 4, ..Stats::default() };
        let picks = vec![
            Pick::Weapon { weapon: WeaponKind::Sword },
            Pick::Slot,
            Pick::Skill { skill: Skill::Potion },
        ];
        set_build(&mut f, stats, picks).unwrap();
        assert!(f.complete(), "{:?}", f.build_issue());
        f
    }

    #[test]
    fn fights_are_deterministic() {
        let a = gen_opponent(3, 1);
        let b = gen_opponent(3, 2);
        let r1 = serde_json::to_string(&fight(&a, &b)).unwrap();
        let r2 = serde_json::to_string(&fight(&a, &b)).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn bots_are_complete() {
        for s in 0..200 {
            let f = gen_opponent(1 + (s % 15) as u32, s);
            assert!(f.complete(), "level {}: {:?}", f.level, f.build_issue());
            assert!(f.plan.len() <= f.slots());
        }
    }

    #[test]
    fn fights_end() {
        for s in 0..300 {
            let a = gen_opponent(1 + (s % 8) as u32, s);
            let b = gen_opponent(1 + (s % 5) as u32, s + 1000);
            let r = fight(&a, &b);
            assert_eq!(r.events.last().unwrap().kind, "end");
        }
    }

    #[test]
    fn gauges_carry_overflow() {
        let mut a = armed("A");
        a.plan.clear();
        a.picks.clear();
        a.rebuild();
        a.stats.end = 60; // long fight
        let mut b = a.clone();
        a.stats.acc = 3;
        b.stats.eva = 0;
        a.stats.eva = 0;
        b.stats.acc = 3;
        let r = fight(&a, &b);
        let a_attacks: Vec<_> = r.events.iter().filter(|e| e.who == 0 && e.kind == "attack").collect();
        let misses = a_attacks.iter().filter(|e| !e.hit).count();
        // Fists 92% - 4% + 3 accuracy * 2% = 94% hit => 6 misses per 100 attacks.
        let expected = a_attacks.len() * 6 / 100;
        assert!(misses == expected || misses == expected + 1, "{misses} misses for {} attacks", a_attacks.len());
    }

    #[test]
    fn stats_cancel_out() {
        // +3 strength against +3 toughness is the same as 0 against 0.
        let mut a = armed("A");
        let mut b = armed("B");
        let base = serde_json::to_string(&fight(&a, &b).events).unwrap();
        a.stats.str += 3;
        b.stats.tou += 3;
        assert_eq!(serde_json::to_string(&fight(&a, &b).events).unwrap(), base);
    }

    #[test]
    fn no_automatic_draw() {
        let mut a = armed("A");
        a.plan.clear();
        let b = armed("B");
        let r = fight(&a, &b);
        assert!(!r.events.iter().any(|e| e.who == 0 && e.kind == "equip"));
    }

    #[test]
    fn plan_matters() {
        let mut smart = armed("Alpha");
        smart.plan = vec![
            Rule { cond: Cond::SelfHpBelow(35), action: Action::Potion },
            Rule { cond: Cond::SelfUnarmed, action: Action::EquipBest },
        ];
        let mut dumb = smart.clone();
        dumb.plan = vec![];
        assert_eq!(fight(&smart, &dumb).winner, 0);
        assert_eq!(fight(&dumb, &smart).winner, 1);
    }

    #[test]
    fn build_rules() {
        let mut f = new_fighter("X");
        assert_eq!(f.slots(), 3);
        assert!(f.build_issue().unwrap().starts_with("Il reste"));
        // Over the cap.
        assert!(set_build(&mut f, Stats { str: 9, ..Stats::default() }, vec![]).is_err());
        // Wrong pick type for level 2.
        f.level = 2;
        assert!(set_build(&mut f, Stats { str: 4, end: 4, spd: 2, ..Stats::default() }, vec![Pick::Slot]).is_err());
        // Locked condition is ignored until unlocked.
        f.level = 3;
        let stats = Stats { str: 4, end: 4, spd: 4, ..Stats::default() };
        set_build(&mut f, stats, vec![Pick::Weapon { weapon: WeaponKind::Club }, Pick::Cond { cond: Cond::Faster }]).unwrap();
        assert!(f.complete());
        assert!(f.knows(Cond::Faster) && !f.knows(Cond::CritReady));
        assert_eq!(f.weapons, vec![WeaponKind::Club]);
    }

    #[test]
    fn old_saves_convert() {
        let old = r#"{"name":"Vieux","level":3,"xp":1,"pending":0,"str":5,"agi":4,"spd":3,"end":4,
            "weapons":["sword"],"skills":["toughness"],"plan":[],"stance":"balanced","wins":2,"losses":1}"#;
        let f: Fighter = serde_json::from_str(old).unwrap();
        assert_eq!((f.level, f.wins), (3, 2));
        assert!(f.weapons.is_empty() && f.stats.sum() == 0);
        assert!(f.build_issue().unwrap().starts_with("Il reste"));
    }

    #[test]
    fn json_roundtrip() {
        let f = handle(r#"{"fn":"new_fighter","args":{"name":"Test"}}"#);
        let v: Value = serde_json::from_str(&f).unwrap();
        let fighter = v["ok"].clone();
        let req = json!({"fn":"info","args":{"fighter":fighter}}).to_string();
        let info: Value = serde_json::from_str(&handle(&req)).unwrap();
        assert_eq!(info["ok"]["budget"], 10);
        let req = json!({"fn":"set_build","args":{"fighter":fighter,"stats":{"str":4,"end":4,"spd":2},"picks":[]}}).to_string();
        let built: Value = serde_json::from_str(&handle(&req)).unwrap();
        let fighter = built["ok"].clone();
        assert_eq!(fighter["stats"]["str"], 4);
        let req = json!({"fn":"fight","args":{"a":fighter,"b":fighter}}).to_string();
        let r: Value = serde_json::from_str(&handle(&req)).unwrap();
        assert!(r["ok"]["events"].as_array().unwrap().len() > 2);
    }
}
