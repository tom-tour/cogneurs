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
    snare: Gauge,
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
            snare: Gauge::default(),
        }
    }

    fn weapon_stats(&self) -> WeaponStats {
        self.wield.map(|w| w.stats()).unwrap_or(FISTS)
    }

    fn rate(&self) -> i32 {
        (12 + self.f.stats.spd * 2 + self.weapon_stats().speed).max(5)
    }

    /// Dodge bonus from the stance.
    fn stance_dodge(&self) -> f32 {
        match self.stance {
            Stance::Aggressive => -0.05,
            Stance::Balanced => 0.0,
            Stance::Defensive => 0.08,
        }
    }

    /// Chance (in %) that an attack of `self` on `foe` is dodged.
    /// Accuracy against evasion: 2% per point of difference.
    fn miss_pct(&self, foe: &Side, acc_bonus: f32) -> i32 {
        let ws = self.weapon_stats();
        let diff = (self.f.stats.acc - foe.f.stats.eva) as f32;
        let hit = (ws.acc - 0.04 + acc_bonus + diff * 0.02 - foe.stance_dodge()).clamp(0.25, 0.97);
        100 - pct(hit)
    }

    /// Critical against composure: 2% per point of difference.
    fn crit_pct(&self, foe: &Side) -> i32 {
        let diff = (self.f.stats.crit - foe.f.stats.cool) as f32;
        pct((self.weapon_stats().crit + diff * 0.02).max(0.0))
    }

    /// Control against tenacity.
    fn control(&self, foe: &Side) -> i32 {
        self.f.stats.ctl - foe.f.stats.ten - if foe.f.has(Skill::Resist) { 2 } else { 0 }
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

    /// Strength against toughness: 8% per point of difference.
    fn damage(&self, ws: WeaponStats, mult: f32, foe: &Side) -> f32 {
        let diff = (self.f.stats.str - foe.f.stats.tou) as f32;
        let mut d = ws.dmg * (1.0 + diff * 0.08).max(0.3) * mult;
        d *= match self.stance {
            Stance::Aggressive => 1.2,
            Stance::Balanced => 1.0,
            Stance::Defensive => 0.85,
        };
        if self.f.has(Skill::Fury) && self.hp_pct_below(30) {
            d *= 1.4;
        }
        if self.f.has(Skill::Bloodlust) && foe.hp_pct_below(50) {
            d *= 1.2;
        }
        if self.f.has(Skill::Relentless) && foe.stunned {
            d *= 1.25;
        }
        if foe.f.has(Skill::LastStand) && foe.hp_pct_below(30) {
            d *= 0.75;
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
        Cond::CritReady => me.crit.ready(me.crit_pct(foe)),
        Cond::EnemyCritReady => foe.crit.ready(foe.crit_pct(me)),
        Cond::Faster => me.rate() > foe.rate(),
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
    for (i, r) in me.f.plan.iter().take(me.f.slots()).enumerate() {
        if me.f.knows(r.cond) && cond_holds(r.cond, me, foe) && action_possible(r.action, me, foe) {
            return (i as i32, r.action);
        }
    }
    // Otherwise: attack with whatever is in hand.
    (-1, Action::Attack)
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

    let crit_pct = s[a].crit_pct(&s[d]);
    let crit = s[a].crit.add(crit_pct);
    let mut mult = o.mult;
    if o.kind == "power" && s[d].f.has(Skill::Brace) {
        mult *= 0.75;
    }
    let mut dmg = s[a].damage(ws, mult, &s[d]);
    if crit {
        let mut c = 1.5;
        if s[a].f.has(Skill::PreciseStrikes) {
            c += 0.5;
        }
        if s[d].f.has(Skill::IceBlood) {
            c -= 0.25;
        }
        dmg *= c;
    }
    let pierces = weapon == Some(WeaponKind::Flail) && !(s[d].guarding && s[d].f.has(Skill::StrongGuard));
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
    let dmg = (dmg.round() as i32).max(1);
    s[d].hp -= dmg;

    let e = push(ev, s, a, o.kind, rule);
    e.weapon = weapon;
    e.hit = true;
    e.crit = crit;
    e.blocked = blocked;
    e.value = dmg;

    if s[d].hp > 0 {
        if weapon == Some(WeaponKind::Club) && !blocked && o.kind != "throw" {
            let stun = 25 + 4 * s[a].control(&s[d]);
            if s[a].stun.add(stun) {
            s[d].stunned = true;
            push(ev, s, d, "stunned", -1);
            }
        }
        if can_counter && blocked && s[d].guarding && s[d].counter.add(60) {
            riposte(s, d, ev);
        }
    }
    true
}

fn riposte(s: &mut [Side; 2], who: usize, ev: &mut Vec<Event>) {
    let ws = s[who].weapon_stats();
    let mult = if s[who].f.has(Skill::SharpRiposte) { 1.0 } else { 0.6 };
    let dmg = (s[who].damage(ws, mult, &s[1 - who]).round() as i32).max(1);
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
        if s[i].f.has(Skill::BreakFree) {
            s[i].gauge += 50;
        }
        push(ev, s, i, "skip", -1);
        return;
    }
    if s[i].f.has(Skill::Regen) {
        s[i].hp = (s[i].hp + 2).min(s[i].max_hp);
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
            // The target's snare gauge fills with the failure chance.
            let ok = 0.75 + s[i].control(&s[f]) as f32 * 0.03 - s[f].stance_dodge() * 0.5;
            let miss = 100 - pct(ok.clamp(0.3, 0.95));
            let hit = !s[f].snare.add(miss);
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
            let ok = (0.6 + s[i].control(&s[f]) as f32 * 0.05).clamp(0.2, 0.9);
            let hit = !s[f].grip.add(100 - pct(ok));
            let w = s[f].wield;
            if hit {
                // With Grip, the weapon falls nearby and can be taken back.
                if let Some(w) = w.filter(|_| !s[f].f.has(Skill::Grip)) {
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

/// Who acts first on equal action gauges: speed, then accuracy, then side A.
fn first_on_tie(s: &[Side; 2]) -> usize {
    let k = |x: &Side| (x.f.stats.spd, x.f.stats.acc);
    if k(&s[1]) > k(&s[0]) {
        1
    } else {
        0
    }
}

/// `a` attacks with its preset `preset`; `b` defends with its defense preset.
pub fn duel(a: &Fighter, preset: usize, b: &Fighter) -> FightResult {
    fight(&a.loadout(preset), &b.loadout(b.defense))
}

/// A fight between two loadouts (see `Fighter::loadout`).
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
