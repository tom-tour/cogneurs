//! Fight simulation, with no randomness at all.
//!
//! Each fighter acts when its action gauge fills up and follows its battle
//! plan: the first rule whose condition holds and whose action is possible is
//! played.
//!
//! Every chance-based effect is an accumulator instead of a dice roll: a 12%
//! dodge chance adds 12 to the dodge gauge each time it is checked, and the
//! dodge happens when the gauge reaches 100. The overflow is kept (99 + 5
//! leaves 4), so over a fight the rates are exact, and both players can see
//! the next dodge or critical hit coming.

use serde::Serialize;

use crate::data::*;

const MAX_ACTIONS: u32 = 200;

/// Gauge values snapshot sent with each event: `[dodge, crit]` per side.
pub type Gauges = [[i32; 2]; 2];

#[derive(Serialize, Debug, Clone)]
pub struct Event {
    pub who: u8,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon: Option<WeaponKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stance: Option<Stance>,
    pub value: i32,
    pub hit: bool,
    pub crit: bool,
    pub blocked: bool,
    /// Index of the plan rule that fired, -1 for the automatic fallback.
    pub rule: i32,
    pub hp: [i32; 2],
    pub g: Gauges,
}

#[derive(Serialize, Debug)]
pub struct FightResult {
    pub winner: u8,
    pub max_hp: [i32; 2],
    pub events: Vec<Event>,
}

/// An accumulator: `add(p)` returns true once every 100/p calls on average.
#[derive(Default, Clone, Copy)]
struct Gauge(i32);

impl Gauge {
    fn add(&mut self, pct: i32) -> bool {
        self.0 += pct.max(0);
        if self.0 >= 100 {
            self.0 -= 100;
            true
        } else {
            false
        }
    }
    fn ready(self, pct: i32) -> bool {
        self.0 + pct.max(0) >= 100
    }
}

struct Side<'a> {
    f: &'a Fighter,
    hp: i32,
    max_hp: i32,
    gauge: i32,
    wield: Option<WeaponKind>,
    inv: Vec<WeaponKind>,
    stance: Stance,
    stunned: bool,
    guarding: bool,
    potions: u8,
    net_ready: bool,
    cd_power: u8,
    cd_disarm: u8,
    turns: u32,
    // Chance accumulators.
    dodge: Gauge,
    crit: Gauge,
    block: Gauge,
    counter: Gauge,
    stun: Gauge,
    grip: Gauge,
}

impl<'a> Side<'a> {
    fn new(f: &'a Fighter) -> Self {
        Side {
            f,
            hp: f.max_hp(),
            max_hp: f.max_hp(),
            gauge: 0,
            wield: None,
            inv: f.weapons.iter().copied().filter(|w| w.wieldable()).collect(),
            stance: f.stance,
            stunned: false,
            guarding: false,
            potions: f.has(Skill::Potion) as u8,
            net_ready: f.weapons.contains(&WeaponKind::Net),
            cd_power: 0,
            cd_disarm: 0,
            turns: 0,
            dodge: Gauge::default(),
            crit: Gauge::default(),
            block: Gauge::default(),
            counter: Gauge::default(),
            stun: Gauge::default(),
            grip: Gauge::default(),
        }
    }

    fn weapon_stats(&self) -> WeaponStats {
        self.wield.map(|w| w.stats()).unwrap_or(FISTS)
    }

    fn rate(&self) -> i32 {
        (10 + self.f.spd * 2 + self.weapon_stats().speed).max(5)
    }

    fn dodge_chance(&self) -> f32 {
        let mut d = 0.04 + self.f.agi as f32 * 0.015;
        if self.f.has(Skill::Evasion) {
            d += 0.10;
        }
        d += match self.stance {
            Stance::Aggressive => -0.05,
            Stance::Balanced => 0.0,
            Stance::Defensive => 0.08,
        };
        d.max(0.0)
    }

    /// Chance (in %) that an attack of `self` on `foe` is dodged.
    fn miss_pct(&self, foe: &Side, acc_bonus: f32) -> i32 {
        let ws = self.weapon_stats();
        let hit = (ws.acc + acc_bonus + self.f.agi as f32 * 0.01 - foe.dodge_chance()).clamp(0.25, 0.97);
        100 - pct(hit)
    }

    fn crit_pct(&self) -> i32 {
        pct(self.weapon_stats().crit + self.f.agi as f32 * 0.005)
    }

    fn hp_pct_below(&self, p: u8) -> bool {
        self.hp * 100 < self.max_hp * p as i32
    }

    fn best_weapon(&self) -> Option<WeaponKind> {
        self.inv
            .iter()
            .copied()
            .max_by(|a, b| a.stats().dmg.partial_cmp(&b.stats().dmg).unwrap())
    }

    fn damage(&self, ws: WeaponStats, mult: f32) -> f32 {
        let mut d = ws.dmg * (1.0 + self.f.str as f32 * 0.08) * mult;
        d *= match self.stance {
            Stance::Aggressive => 1.2,
            Stance::Balanced => 1.0,
            Stance::Defensive => 0.85,
        };
        if self.f.has(Skill::Fury) && self.hp_pct_below(30) {
            d *= 1.4;
        }
        d
    }
}

fn pct(p: f32) -> i32 {
    (p * 100.0).round() as i32
}

fn cond_holds(c: Cond, me: &Side, foe: &Side) -> bool {
    match c {
        Cond::Always => true,
        Cond::FirstTurn => me.turns == 0,
        Cond::SelfHpBelow(p) => me.hp_pct_below(p),
        Cond::EnemyHpBelow(p) => foe.hp_pct_below(p),
        Cond::SelfUnarmed => me.wield.is_none(),
        Cond::SelfArmed => me.wield.is_some(),
        Cond::EnemyArmed => foe.wield.is_some(),
        Cond::EnemyUnarmed => foe.wield.is_none(),
        Cond::EnemyStunned => foe.stunned,
        Cond::EnemyGuarding => foe.guarding,
        Cond::EnemyDodgeReady => foe.dodge.ready(me.miss_pct(foe, 0.0)),
        Cond::CritReady => me.crit.ready(me.crit_pct()),
    }
}

fn action_possible(a: Action, me: &Side, foe: &Side) -> bool {
    match a {
        Action::Attack => true,
        Action::EquipBest => me.best_weapon().is_some_and(|w| me.wield != Some(w)),
        Action::Equip(w) => w.wieldable() && me.inv.contains(&w) && me.wield != Some(w),
        Action::Throw => me.wield.is_some(),
        Action::ThrowNet => me.net_ready && !foe.stunned,
        Action::PowerStrike => me.f.has(Skill::PowerStrike) && me.cd_power == 0,
        Action::Disarm => me.f.has(Skill::Disarm) && me.cd_disarm == 0 && foe.wield.is_some(),
        Action::Guard => true,
        Action::Potion => me.potions > 0 && me.hp < me.max_hp,
        Action::Stance(s) => me.stance != s,
    }
}

fn choose(me: &Side, foe: &Side) -> (i32, Action) {
    let slots = plan_slots(me.f.level);
    for (i, r) in me.f.plan.iter().take(slots).enumerate() {
        if cond_holds(r.cond, me, foe) && action_possible(r.action, me, foe) {
            return (i as i32, r.action);
        }
    }
    if me.wield.is_none() && me.best_weapon().is_some() {
        (-1, Action::EquipBest)
    } else {
        (-1, Action::Attack)
    }
}

fn snapshot(s: &[Side; 2]) -> ([i32; 2], Gauges) {
    (
        [s[0].hp.max(0), s[1].hp.max(0)],
        [[s[0].dodge.0, s[0].crit.0], [s[1].dodge.0, s[1].crit.0]],
    )
}

fn push<'e>(events: &'e mut Vec<Event>, s: &[Side; 2], who: usize, kind: &'static str, rule: i32) -> &'e mut Event {
    let (hp, g) = snapshot(s);
    events.push(Event {
        who: who as u8,
        kind,
        weapon: None,
        stance: None,
        value: 0,
        hit: false,
        crit: false,
        blocked: false,
        rule,
        hp,
        g,
    });
    events.last_mut().unwrap()
}

struct StrikeOpts {
    kind: &'static str,
    mult: f32,
    acc_bonus: f32,
    counterable: bool,
}

/// Resolve one attack from `a` on `1 - a`. Returns whether it hit.
fn strike(s: &mut [Side; 2], a: usize, ev: &mut Vec<Event>, rule: i32, o: StrikeOpts) -> bool {
    let d = 1 - a;
    let weapon = s[a].wield;
    let ws = s[a].weapon_stats();
    let miss = s[a].miss_pct(&s[d], o.acc_bonus);
    let can_counter = o.counterable && weapon != Some(WeaponKind::Spear) && s[d].f.has(Skill::Counter);

    if s[d].dodge.add(miss) {
        let riposte_now = can_counter && s[d].counter.add(40);
        let e = push(ev, s, a, o.kind, rule);
        e.weapon = weapon;
        if riposte_now {
            riposte(s, d, ev);
        }
        return false;
    }

    let crit_pct = s[a].crit_pct();
    let crit = s[a].crit.add(crit_pct);
    let mut dmg = s[a].damage(ws, o.mult);
    if crit {
        dmg *= 1.5;
    }
    let pierces = weapon == Some(WeaponKind::Flail);
    let shield = s[d].f.has(Skill::Shield);
    let mut blocked = false;
    if !pierces {
        if s[d].guarding {
            dmg *= if shield { 0.25 } else { 0.5 };
            blocked = true;
        } else if shield && s[d].block.add(12) {
            dmg *= 0.25;
            blocked = true;
        }
    }
    if s[d].f.has(Skill::Toughness) {
        dmg -= 2.0;
    }
    let dmg = (dmg.round() as i32).max(1);
    s[d].hp -= dmg;

    let e = push(ev, s, a, o.kind, rule);
    e.weapon = weapon;
    e.hit = true;
    e.crit = crit;
    e.blocked = blocked;
    e.value = dmg;

    if s[d].hp > 0 {
        if weapon == Some(WeaponKind::Club) && !blocked && o.kind != "throw" && s[a].stun.add(25) {
            s[d].stunned = true;
            push(ev, s, d, "stunned", -1);
        }
        if can_counter && blocked && s[d].guarding && s[d].counter.add(60) {
            riposte(s, d, ev);
        }
    }
    true
}

fn riposte(s: &mut [Side; 2], who: usize, ev: &mut Vec<Event>) {
    let ws = s[who].weapon_stats();
    let dmg = (s[who].damage(ws, 0.6).round() as i32).max(1);
    s[1 - who].hp -= dmg;
    let weapon = s[who].wield;
    let e = push(ev, s, who, "counter", -1);
    e.weapon = weapon;
    e.hit = true;
    e.value = dmg;
}

fn take_turn(s: &mut [Side; 2], i: usize, ev: &mut Vec<Event>) {
    let f = 1 - i;
    s[i].gauge -= 100;
    s[i].guarding = false;
    if s[i].stunned {
        s[i].stunned = false;
        push(ev, s, i, "skip", -1);
        return;
    }
    s[i].cd_power = s[i].cd_power.saturating_sub(1);
    s[i].cd_disarm = s[i].cd_disarm.saturating_sub(1);

    let (rule, action) = choose(&s[i], &s[f]);
    s[i].turns += 1;

    match action {
        Action::Attack => {
            strike(s, i, ev, rule, StrikeOpts { kind: "attack", mult: 1.0, acc_bonus: 0.0, counterable: true });
        }
        Action::PowerStrike => {
            s[i].cd_power = 3;
            strike(s, i, ev, rule, StrikeOpts { kind: "power", mult: 2.0, acc_bonus: -0.05, counterable: true });
        }
        Action::Throw => {
            // Thrown with the weapon's stats, then it is gone.
            let w = s[i].wield;
            strike(s, i, ev, rule, StrikeOpts { kind: "throw", mult: 1.4, acc_bonus: 0.05, counterable: false });
            s[i].wield = None;
            if let Some(w) = w {
                s[i].inv.retain(|x| *x != w);
            }
        }
        Action::Equip(_) | Action::EquipBest => {
            let w = match action {
                Action::Equip(w) => w,
                _ => s[i].best_weapon().unwrap(),
            };
            s[i].wield = Some(w);
            s[i].gauge += 50; // drawing a weapon is a quick action
            let e = push(ev, s, i, "equip", rule);
            e.weapon = Some(w);
        }
        Action::ThrowNet => {
            s[i].net_ready = false;
            // The net uses the target's dodge gauge, at half its dodge chance.
            let miss = 100 - pct((0.8 - s[f].dodge_chance() * 0.5).clamp(0.3, 0.95));
            let hit = !s[f].dodge.add(miss);
            if hit {
                s[f].stunned = true;
            }
            let e = push(ev, s, i, "net", rule);
            e.weapon = Some(WeaponKind::Net);
            e.hit = hit;
        }
        Action::Disarm => {
            s[i].cd_disarm = 3;
            // The target's grip gauge fills with the failure chance.
            let ok = (0.6 + (s[i].f.agi - s[f].f.agi) as f32 * 0.05).clamp(0.2, 0.9);
            let hit = !s[f].grip.add(100 - pct(ok));
            let w = s[f].wield;
            if hit {
                if let Some(w) = w {
                    s[f].inv.retain(|x| *x != w);
                }
                s[f].wield = None;
            }
            let e = push(ev, s, i, "disarm", rule);
            e.weapon = w;
            e.hit = hit;
        }
        Action::Guard => {
            s[i].guarding = true;
            push(ev, s, i, "guard", rule);
        }
        Action::Potion => {
            s[i].potions -= 1;
            let heal = (s[i].max_hp * 2 / 5).min(s[i].max_hp - s[i].hp);
            s[i].hp += heal;
            let e = push(ev, s, i, "potion", rule);
            e.value = heal;
        }
        Action::Stance(st) => {
            s[i].stance = st;
            s[i].gauge += 50;
            let e = push(ev, s, i, "stance", rule);
            e.stance = Some(st);
        }
    }
    // Keep the final HP and gauges of the turn on its last event.
    let (hp, g) = snapshot(s);
    if let Some(e) = ev.last_mut() {
        e.hp = hp;
        e.g = g;
    }
}

/// Who acts first on equal action gauges: speed, then agility, then side A.
fn first_on_tie(s: &[Side; 2]) -> usize {
    let k = |x: &Side| (x.f.spd, x.f.agi);
    if k(&s[1]) > k(&s[0]) {
        1
    } else {
        0
    }
}

pub fn fight(a: &Fighter, b: &Fighter) -> FightResult {
    let mut s = [Side::new(a), Side::new(b)];
    let max_hp = [s[0].max_hp, s[1].max_hp];
    let mut ev = Vec::new();

    for _ in 0..MAX_ACTIONS {
        while s[0].gauge < 100 && s[1].gauge < 100 {
            s[0].gauge += s[0].rate();
            s[1].gauge += s[1].rate();
        }
        let i = match s[0].gauge.cmp(&s[1].gauge) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => first_on_tie(&s),
        };
        take_turn(&mut s, i, &mut ev);
        if s[0].hp <= 0 || s[1].hp <= 0 {
            break;
        }
    }

    let winner = if s[1].hp <= 0 {
        0
    } else if s[0].hp <= 0 {
        1
    } else if s[0].hp * s[1].max_hp >= s[1].hp * s[0].max_hp {
        0
    } else {
        1
    };
    push(&mut ev, &s, winner as usize, "end", -1);
    FightResult { winner, max_hp, events: ev }
}
