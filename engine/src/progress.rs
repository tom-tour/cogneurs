//! Character creation, builds, levels and opponent generation.
//!
//! Progression has no randomness: the player spends every stat point and
//! makes every level choice, and may redo them for free between fights.
//! Only bots and the offered opponents are generated at random.

use serde::Serialize;

use crate::data::*;
use crate::rng::{hash_str, Rng};

pub fn default_plan() -> Vec<Rule> {
    vec![Rule { cond: Cond::SelfUnarmed, action: Action::EquipBest }]
}

/// A fresh level 1 fighter, with its starting points still to spend.
pub fn new_fighter(name: &str) -> Fighter {
    Fighter {
        name: name.to_string(),
        level: 1,
        xp: 0,
        stats: Stats::default(),
        picks: Vec::new(),
        weapons: Vec::new(),
        skills: Vec::new(),
        plan: default_plan(),
        stance: Stance::Balanced,
        wins: 0,
        losses: 0,
    }
}

/// Everything the page needs to edit a build.
#[derive(Serialize)]
pub struct BuildInfo {
    pub budget: i32,
    pub spent: i32,
    pub cap: i32,
    pub slots: usize,
    pub xp_needed: u32,
    pub max_hp: i32,
    pub issue: Option<String>,
    /// Choices offered on odd levels (rule slot or condition).
    pub odd: Vec<Pick>,
    /// Choices offered on even levels (weapon or skill).
    pub even: Vec<Pick>,
}

pub fn odd_choices() -> Vec<Pick> {
    std::iter::once(Pick::Slot).chain(UNLOCKABLE_CONDS.iter().map(|c| Pick::Cond { cond: *c })).collect()
}

pub fn even_choices() -> Vec<Pick> {
    ALL_WEAPONS
        .iter()
        .map(|w| Pick::Weapon { weapon: *w })
        .chain(ALL_SKILLS.iter().map(|s| Pick::Skill { skill: *s }))
        .collect()
}

pub fn build_info(f: &Fighter) -> BuildInfo {
    BuildInfo {
        budget: stat_budget(f.level),
        spent: f.stats.sum(),
        cap: stat_cap(f.level),
        slots: f.slots(),
        xp_needed: xp_needed(f.level),
        max_hp: f.max_hp(),
        issue: f.build_issue(),
        odd: odd_choices(),
        even: even_choices(),
    }
}

/// Replaces the stats and level choices (creation, level-up or respec).
/// Partial builds are accepted, as long as nothing is over the limits.
pub fn set_build(f: &mut Fighter, stats: Stats, picks: Vec<Pick>) -> Result<(), String> {
    let mut g = f.clone();
    g.stats = stats;
    g.picks = picks;
    g.rebuild();
    // Limit errors come first; "Il reste..." only means unfinished.
    if let Some(issue) = g.build_issue().filter(|i| !i.starts_with("Il reste")) {
        return Err(issue);
    }
    *f = g;
    Ok(())
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
    if f.weapons.iter().any(|w| w.wieldable()) {
        plan.push(Rule { cond: Cond::SelfUnarmed, action: Action::EquipBest });
    }
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
        let mut conds = vec![Cond::Always, Cond::EnemyHpBelow(50)];
        if f.knows(Cond::EnemyStunned) {
            conds.push(Cond::EnemyStunned);
        }
        plan.push(Rule { cond: *rng.pick(&conds), action: Action::PowerStrike });
    }
    if f.has(Skill::Shield) && rng.chance(0.5) {
        let cond = if f.knows(Cond::EnemyCritReady) { Cond::EnemyCritReady } else { Cond::SelfHpBelow(50) };
        plan.push(Rule { cond, action: Action::Guard });
    }
    if f.weapons.iter().any(|w| w.wieldable()) && rng.chance(0.3) {
        plan.push(Rule { cond: Cond::EnemyHpBelow(25), action: Action::Throw });
    }
    plan.truncate(f.slots());
    plan
}

/// Random valid build for a bot of the given level.
fn random_build(f: &mut Fighter, rng: &mut Rng) {
    let cap = stat_cap(f.level);
    let mut left = stat_budget(f.level);
    while left > 0 {
        let v = f.stats.values_mut();
        let i = rng.below(10);
        if *v[i] < cap {
            *v[i] += 1;
            left -= 1;
        }
    }
    f.picks.clear();
    for level in 2..=f.level {
        let pool: Vec<Pick> = if level % 2 == 1 { odd_choices() } else { even_choices() }
            .into_iter()
            .filter(|p| *p == Pick::Slot || !f.picks.contains(p))
            .filter(|p| *p != Pick::Slot || f.slots() < MAX_RULES)
            .collect();
        if pool.is_empty() {
            break;
        }
        let p = *rng.pick(&pool);
        f.picks.push(p);
        f.rebuild();
    }
}

pub fn gen_opponent(level: u32, seed: u64) -> Fighter {
    let mut rng = Rng::new(seed);
    let name = gen_name(&mut rng);
    let mut f = new_fighter(&name);
    f.level = level.max(1);
    random_build(&mut f, &mut rng);
    f.stance = *rng.pick(&[Stance::Aggressive, Stance::Balanced, Stance::Balanced, Stance::Defensive]);
    f.plan = ai_plan(&f, &mut rng);
    f
}

/// Fights are fully determined, so a prediction cannot just replay the real
/// fight: the enemy plan is hidden. It plays `a` against `n` plausible plans
/// for `b` (built from b's visible gear) and returns the share of wins.
pub fn predict(a: &Fighter, b: &Fighter, n: u32, seed: u64) -> f32 {
    let mut rng = Rng::new(seed ^ hash_str(&b.name));
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
