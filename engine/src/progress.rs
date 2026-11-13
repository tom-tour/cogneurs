//! Character creation, level-ups and opponent generation.

use serde::{Deserialize, Serialize};

use crate::data::*;
use crate::rng::{hash_str, Rng};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum Upgrade {
    Stat { stat: Stat, amount: i32 },
    Weapon { weapon: WeaponKind },
    Skill { skill: Skill },
}

pub fn default_plan() -> Vec<Rule> {
    vec![
        Rule { cond: Cond::SelfUnarmed, action: Action::EquipBest },
        Rule { cond: Cond::Always, action: Action::Attack },
    ]
}

pub fn new_fighter(name: &str, seed: u64) -> Fighter {
    let mut rng = Rng::new(seed ^ hash_str(name));
    let mut f = Fighter {
        name: name.to_string(),
        level: 1,
        xp: 0,
        pending: 1,
        str: 3,
        agi: 3,
        spd: 3,
        end: 3,
        weapons: Vec::new(),
        skills: Vec::new(),
        plan: default_plan(),
        stance: Stance::Balanced,
        wins: 0,
        losses: 0,
    };
    for _ in 0..4 {
        *f.stat_mut(*rng.pick(&ALL_STATS)) += 1;
    }
    f
}

fn random_upgrade(f: &Fighter, rng: &mut Rng) -> Upgrade {
    let r = rng.f32();
    if r < 0.30 {
        let free: Vec<_> = ALL_WEAPONS.iter().filter(|w| !f.weapons.contains(w)).collect();
        if !free.is_empty() {
            return Upgrade::Weapon { weapon: **rng.pick(&free) };
        }
    } else if r < 0.60 {
        let free: Vec<_> = ALL_SKILLS.iter().filter(|s| !f.has(**s)).collect();
        if !free.is_empty() {
            return Upgrade::Skill { skill: **rng.pick(&free) };
        }
    }
    let amount = if rng.chance(0.3) { 3 } else { 2 };
    Upgrade::Stat { stat: *rng.pick(&ALL_STATS), amount }
}

/// Three distinct upgrades to choose from. Deterministic for a given
/// fighter state, so reloading the page never rerolls them.
pub fn level_up_options(f: &Fighter) -> Vec<Upgrade> {
    let seed = hash_str(&f.name) ^ ((f.level as u64) << 32) ^ (f.pending as u64) ^ ((f.wins + f.losses) as u64) << 16;
    let mut rng = Rng::new(seed);
    let mut out: Vec<Upgrade> = Vec::new();
    let mut guard = 0;
    while out.len() < 3 && guard < 100 {
        guard += 1;
        let u = random_upgrade(f, &mut rng);
        let dup = out.iter().any(|o| match (o, &u) {
            (Upgrade::Stat { stat: a, .. }, Upgrade::Stat { stat: b, .. }) => a == b,
            _ => *o == u,
        });
        if !dup {
            out.push(u);
        }
    }
    out
}

pub fn apply(f: &mut Fighter, u: Upgrade) {
    match u {
        Upgrade::Stat { stat, amount } => *f.stat_mut(stat) += amount,
        Upgrade::Weapon { weapon } => {
            if !f.weapons.contains(&weapon) {
                f.weapons.push(weapon)
            }
        }
        Upgrade::Skill { skill } => {
            if !f.has(skill) {
                f.skills.push(skill)
            }
        }
    }
    f.pending = f.pending.saturating_sub(1);
}

pub fn record(f: &mut Fighter, won: bool) {
    if won {
        f.wins += 1;
        f.xp += 2;
    } else {
        f.losses += 1;
        f.xp += 1;
    }
    while f.xp >= xp_needed(f.level) {
        f.xp -= xp_needed(f.level);
        f.level += 1;
        f.pending += 1;
    }
}

const SYL: [&str; 20] = [
    "gro", "bak", "zor", "mil", "tur", "ka", "dru", "pom", "vex", "lo", "rib", "ska", "nu", "fer", "gal", "tok",
    "bri", "mo", "zul", "dar",
];

fn gen_name(rng: &mut Rng) -> String {
    let n = 2 + rng.below(2);
    let mut s: String = (0..n).map(|_| *rng.pick(&SYL)).collect();
    if let Some(c) = s.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    s
}

/// A reasonable plan for a computer-controlled fighter.
pub fn ai_plan(f: &Fighter, rng: &mut Rng) -> Vec<Rule> {
    let mut plan = Vec::new();
    if f.has(Skill::Potion) {
        plan.push(Rule { cond: Cond::SelfHpBelow(*rng.pick(&[25u8, 40, 50])), action: Action::Potion });
    }
    if f.weapons.contains(&WeaponKind::Net) && rng.chance(0.7) {
        plan.push(Rule { cond: Cond::FirstTurn, action: Action::ThrowNet });
    }
    if f.has(Skill::Disarm) {
        plan.push(Rule { cond: Cond::EnemyArmed, action: Action::Disarm });
    }
    if f.has(Skill::PowerStrike) {
        let cond = *rng.pick(&[Cond::Always, Cond::EnemyStunned, Cond::EnemyHpBelow(50)]);
        plan.push(Rule { cond, action: Action::PowerStrike });
    }
    if f.has(Skill::Shield) && rng.chance(0.5) {
        plan.push(Rule { cond: Cond::SelfHpBelow(50), action: Action::Guard });
    }
    if f.weapons.iter().any(|w| w.wieldable()) {
        plan.insert(0, Rule { cond: Cond::SelfUnarmed, action: Action::EquipBest });
        if rng.chance(0.3) {
            plan.push(Rule { cond: Cond::EnemyHpBelow(25), action: Action::Throw });
        }
    }
    plan.truncate(plan_slots(f.level) - 1);
    plan.push(Rule { cond: Cond::Always, action: Action::Attack });
    plan
}

pub fn gen_opponent(level: u32, seed: u64) -> Fighter {
    let mut rng = Rng::new(seed);
    let name = gen_name(&mut rng);
    let mut f = new_fighter(&name, rng.next_u64());
    f.pending = 0;
    f.plan.clear();
    for _ in 0..level {
        let u = random_upgrade(&f, &mut rng);
        f.pending = 1;
        apply(&mut f, u);
    }
    f.level = level;
    f.stance = *rng.pick(&[Stance::Aggressive, Stance::Balanced, Stance::Balanced, Stance::Defensive]);
    f.plan = ai_plan(&f, &mut rng);
    f
}

/// Fights are fully determined, so a prediction cannot just replay the real
/// fight: the enemy plan is hidden. It plays `a` against `n` plausible plans
/// for `b` (built from b's visible gear) and returns the share of wins.
pub fn predict(a: &Fighter, b: &Fighter, n: u32, seed: u64) -> f32 {
    let mut rng = Rng::new(seed);
    let mut guess = b.clone();
    let wins = (0..n)
        .filter(|_| {
            guess.plan = ai_plan(b, &mut rng);
            crate::combat::fight(a, &guess).winner == 0
        })
        .count();
    wins as f32 / n.max(1) as f32
}

/// What other players may see of a fighter: everything but its plan.
pub fn public_view(f: &Fighter) -> Fighter {
    let mut p = f.clone();
    p.plan.clear();
    p
}

/// Plans longer than the maximum slot count are cut.
pub fn sanitize_plan(plan: &mut Vec<Rule>) {
    plan.truncate(MAX_RULES);
}

pub fn gen_opponents(f: &Fighter, seed: u64) -> Vec<Fighter> {
    let mut rng = Rng::new(seed);
    let l = f.level;
    [l.saturating_sub(1).max(1), l, l + 1].iter().map(|lv| gen_opponent(*lv, rng.next_u64())).collect()
}
