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
use progress::Upgrade;

#[derive(Deserialize)]
#[serde(tag = "fn", content = "args", rename_all = "snake_case")]
enum Call {
    NewFighter { name: String, seed: u64 },
    Options { fighter: Fighter },
    Apply { fighter: Fighter, upgrade: Upgrade },
    Record { fighter: Fighter, won: bool },
    Opponents { fighter: Fighter, seed: u64 },
    Fight { a: Fighter, b: Fighter },
    Predict { a: Fighter, b: Fighter, n: u32, seed: u64 },
    Meta { level: u32 },
}

pub fn handle(request: &str) -> String {
    let call: Call = match serde_json::from_str(request) {
        Ok(c) => c,
        Err(e) => return json!({ "err": e.to_string() }).to_string(),
    };
    let ok: Value = match call {
        Call::NewFighter { name, seed } => json!(progress::new_fighter(&name, seed)),
        Call::Options { fighter } => json!(progress::level_up_options(&fighter)),
        Call::Apply { mut fighter, upgrade } => {
            progress::apply(&mut fighter, upgrade);
            json!(fighter)
        }
        Call::Record { mut fighter, won } => {
            progress::record(&mut fighter, won);
            json!(fighter)
        }
        Call::Opponents { fighter, seed } => json!(progress::gen_opponents(&fighter, seed)),
        Call::Fight { a, b } => json!(combat::fight(&a, &b)),
        Call::Predict { a, b, n, seed } => json!(progress::predict(&a, &b, n.min(500), seed)),
        Call::Meta { level } => json!({ "slots": plan_slots(level), "xp_needed": xp_needed(level) }),
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

    fn armed(name: &str) -> Fighter {
        let mut f = new_fighter(name, 1);
        f.pending = 0;
        f.level = 4;
        f.weapons = vec![WeaponKind::Sword];
        f.skills = vec![Skill::Potion];
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
        // 20% dodge chance: exactly one dodge every 5 attacks.
        let mut a = armed("A");
        a.plan = vec![Rule { cond: Cond::Always, action: Action::Attack }];
        a.weapons.clear();
        a.skills.clear();
        a.end = 60; // long fight
        a.agi = 3;
        let mut b = a.clone();
        b.agi = 0;
        let r = fight(&a, &b);
        let a_attacks: Vec<_> = r.events.iter().filter(|e| e.who == 0 && e.kind == "attack").collect();
        let misses = a_attacks.iter().filter(|e| !e.hit).count();
        // Fists 92% + agi 3% - dodge 4% = 91% hit => 9 misses per 100 attacks.
        let expected = a_attacks.len() * 9 / 100;
        assert!(misses == expected || misses == expected + 1, "{misses} misses for {} attacks", a_attacks.len());
    }

    #[test]
    fn plan_matters() {
        let mut smart = armed("Alpha");
        smart.plan = vec![
            Rule { cond: Cond::SelfHpBelow(35), action: Action::Potion },
            Rule { cond: Cond::SelfUnarmed, action: Action::EquipBest },
            Rule { cond: Cond::Always, action: Action::Attack },
        ];
        let mut dumb = smart.clone();
        dumb.plan = vec![Rule { cond: Cond::Always, action: Action::Attack }];
        // Smart wins from both sides of the arena.
        assert_eq!(fight(&smart, &dumb).winner, 0);
        assert_eq!(fight(&dumb, &smart).winner, 1);
    }

    #[test]
    fn json_roundtrip() {
        let f = handle(r#"{"fn":"new_fighter","args":{"name":"Test","seed":3}}"#);
        let v: Value = serde_json::from_str(&f).unwrap();
        let fighter = v["ok"].clone();
        let req = json!({"fn":"options","args":{"fighter":fighter}}).to_string();
        let opts: Value = serde_json::from_str(&handle(&req)).unwrap();
        assert_eq!(opts["ok"].as_array().unwrap().len(), 3);
        let req = json!({"fn":"fight","args":{"a":fighter,"b":fighter}}).to_string();
        let r: Value = serde_json::from_str(&handle(&req)).unwrap();
        assert!(r["ok"]["events"].as_array().unwrap().len() > 2);
        let req = json!({"fn":"predict","args":{"a":fighter,"b":fighter,"n":20,"seed":1}}).to_string();
        let p: Value = serde_json::from_str(&handle(&req)).unwrap();
        assert!(p["ok"].as_f64().is_some());
    }
}
