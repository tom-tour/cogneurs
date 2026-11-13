//! Game data: fighters, weapons, skills, stats and battle plans.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum WeaponKind {
    Knife,
    Sword,
    Club,
    Spear,
    Flail,
    Net,
}

pub const ALL_WEAPONS: [WeaponKind; 6] = [
    WeaponKind::Knife,
    WeaponKind::Sword,
    WeaponKind::Club,
    WeaponKind::Spear,
    WeaponKind::Flail,
    WeaponKind::Net,
];

#[derive(Clone, Copy, Debug)]
pub struct WeaponStats {
    pub dmg: f32,
    pub acc: f32,
    /// Added to the action gauge every tick.
    pub speed: i32,
    pub crit: f32,
}

pub const FISTS: WeaponStats = WeaponStats { dmg: 3.0, acc: 0.92, speed: 2, crit: 0.05 };

impl WeaponKind {
    pub fn stats(self) -> WeaponStats {
        match self {
            WeaponKind::Knife => WeaponStats { dmg: 5.0, acc: 0.95, speed: 4, crit: 0.20 },
            WeaponKind::Sword => WeaponStats { dmg: 8.0, acc: 0.88, speed: 0, crit: 0.08 },
            WeaponKind::Club => WeaponStats { dmg: 11.0, acc: 0.78, speed: -4, crit: 0.05 },
            WeaponKind::Spear => WeaponStats { dmg: 7.0, acc: 0.90, speed: 0, crit: 0.06 },
            WeaponKind::Flail => WeaponStats { dmg: 9.0, acc: 0.82, speed: -2, crit: 0.06 },
            // The net is a throwable item, never wielded.
            WeaponKind::Net => WeaponStats { dmg: 0.0, acc: 0.0, speed: 0, crit: 0.0 },
        }
    }

    pub fn wieldable(self) -> bool {
        self != WeaponKind::Net
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Skill {
    // Power
    /// Double damage strike, 3 turns cooldown.
    PowerStrike,
    // Rampart
    /// Passive 12% block chance; the Guard action blocks 75% instead of 50%.
    Shield,
    /// Guarding also stops the flail.
    StrongGuard,
    /// Power strikes taken deal x1.5 instead of x2.
    Brace,
    // Finesse
    /// Critical hits deal x2 instead of x1.5.
    PreciseStrikes,
    // Composure
    /// Critical hits taken deal 0.25 less.
    IceBlood,
    /// Riposte after a dodge or a guarded hit.
    Counter,
    /// Ripostes deal full damage instead of 60%.
    SharpRiposte,
    // Control
    /// Knock the enemy weapon away (it is lost for the fight).
    Disarm,
    // Tenacity
    /// Counts as 2 more tenacity against control.
    Resist,
    /// A stun only costs half a turn.
    BreakFree,
    /// A disarmed weapon stays in the inventory and can be taken back.
    Grip,
    // Rage
    /// +40% damage under 30% HP.
    Fury,
    /// +20% damage against an enemy under 50% HP.
    Bloodlust,
    /// +25% damage against a stunned enemy.
    Relentless,
    // Survival
    /// One potion per fight, heals 40% of max HP.
    Potion,
    /// +2 HP at the start of each of its turns.
    Regen,
    /// -25% damage taken under 30% HP.
    LastStand,
}

/// Hits landed with a weapon needed to reach each mastery tier. Tier 1 gives
/// +3% accuracy with it, tier 2 +10% damage, tier 3 +5% critical chance.
pub const MASTERY_TIERS: [u32; 3] = [10, 30, 60];

pub fn mastery_tier(hits: u32) -> usize {
    MASTERY_TIERS.iter().filter(|t| hits >= **t).count()
}

/// Bonus for carrying two items together. Each offensive synergy has a
/// defensive one that cancels it exactly.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Synergy {
    /// Club + power strike: power strikes deal +15% damage.
    Crush,
    /// Shield + brace: power strikes taken deal 15% less.
    Unshakable,
    /// Sword + precise strikes: +10% critical chance.
    KeenBlade,
    /// Ice blood + riposte: -10% critical chance for the enemy.
    CalmEye,
    /// Spear + disarm: +3 control.
    FirmHold,
    /// Resist + grip: +3 tenacity against control.
    IronWill,
    /// Fury + bloodlust: +15% damage against an enemy under 50% HP.
    Quarry,
    /// Potion + last stand: 15% less damage taken under 50% HP.
    SecondWind,
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct SynergyDef {
    pub key: Synergy,
    pub needs: [Pick; 2],
    /// The synergy this one cancels (or is cancelled by).
    pub answers: Synergy,
    pub offense: bool,
}

const fn syn(key: Synergy, needs: [Pick; 2], answers: Synergy, offense: bool) -> SynergyDef {
    SynergyDef { key, needs, answers, offense }
}

/// Synergies in pairs: the offensive one, then the defensive one answering it.
pub const SYNERGIES: [SynergyDef; 8] = [
    syn(Synergy::Crush, [w(WeaponKind::Club), sk(Skill::PowerStrike)], Synergy::Unshakable, true),
    syn(Synergy::Unshakable, [sk(Skill::Shield), sk(Skill::Brace)], Synergy::Crush, false),
    syn(Synergy::KeenBlade, [w(WeaponKind::Sword), sk(Skill::PreciseStrikes)], Synergy::CalmEye, true),
    syn(Synergy::CalmEye, [sk(Skill::IceBlood), sk(Skill::Counter)], Synergy::KeenBlade, false),
    syn(Synergy::FirmHold, [w(WeaponKind::Spear), sk(Skill::Disarm)], Synergy::IronWill, true),
    syn(Synergy::IronWill, [sk(Skill::Resist), sk(Skill::Grip)], Synergy::FirmHold, false),
    syn(Synergy::Quarry, [sk(Skill::Fury), sk(Skill::Bloodlust)], Synergy::SecondWind, true),
    syn(Synergy::SecondWind, [sk(Skill::Potion), sk(Skill::LastStand)], Synergy::Quarry, false),
];

/// A skill tree branch: three nodes, each with its prerequisite.
#[derive(Serialize, Clone, Copy, Debug)]
pub struct Branch {
    pub key: &'static str,
    pub nodes: [Node; 3],
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct Node {
    pub pick: Pick,
    pub req: Option<Pick>,
}

const fn w(weapon: WeaponKind) -> Pick {
    Pick::Weapon { weapon }
}
const fn sk(skill: Skill) -> Pick {
    Pick::Skill { skill }
}
const fn root(pick: Pick) -> Node {
    Node { pick, req: None }
}
const fn after(pick: Pick, req: Pick) -> Node {
    Node { pick, req: Some(req) }
}

/// The skill tree, in opposed pairs: each offensive branch is followed by
/// the defensive branch that answers it.
pub const TREE: [Branch; 8] = [
    Branch {
        key: "power",
        nodes: [
            root(w(WeaponKind::Club)),
            after(w(WeaponKind::Flail), w(WeaponKind::Club)),
            after(sk(Skill::PowerStrike), w(WeaponKind::Club)),
        ],
    },
    Branch {
        key: "rampart",
        nodes: [
            root(sk(Skill::Shield)),
            after(sk(Skill::StrongGuard), sk(Skill::Shield)),
            after(sk(Skill::Brace), sk(Skill::Shield)),
        ],
    },
    Branch {
        key: "finesse",
        nodes: [
            root(w(WeaponKind::Sword)),
            after(w(WeaponKind::Knife), w(WeaponKind::Sword)),
            after(sk(Skill::PreciseStrikes), w(WeaponKind::Sword)),
        ],
    },
    Branch {
        key: "composure",
        nodes: [
            root(sk(Skill::IceBlood)),
            after(sk(Skill::Counter), sk(Skill::IceBlood)),
            after(sk(Skill::SharpRiposte), sk(Skill::Counter)),
        ],
    },
    Branch {
        key: "control",
        nodes: [
            root(w(WeaponKind::Spear)),
            after(w(WeaponKind::Net), w(WeaponKind::Spear)),
            after(sk(Skill::Disarm), w(WeaponKind::Spear)),
        ],
    },
    Branch {
        key: "tenacity",
        nodes: [
            root(sk(Skill::Resist)),
            after(sk(Skill::BreakFree), sk(Skill::Resist)),
            after(sk(Skill::Grip), sk(Skill::Resist)),
        ],
    },
    Branch {
        key: "rage",
        nodes: [
            root(sk(Skill::Fury)),
            after(sk(Skill::Bloodlust), sk(Skill::Fury)),
            after(sk(Skill::Relentless), sk(Skill::Fury)),
        ],
    },
    Branch {
        key: "survival",
        nodes: [
            root(sk(Skill::Potion)),
            after(sk(Skill::Regen), sk(Skill::Potion)),
            after(sk(Skill::LastStand), sk(Skill::Potion)),
        ],
    },
];

/// Prerequisite of a tree pick, if any.
pub fn requirement(p: Pick) -> Option<Pick> {
    TREE.iter().flat_map(|b| b.nodes.iter()).find(|n| n.pick == p).and_then(|n| n.req)
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Aggressive,
    #[default]
    Balanced,
    Defensive,
}

/// Stats come in opposed pairs: one attack point cancels exactly one defense
/// point, since every formula uses the difference. Speed and endurance stand
/// alone.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(default)]
pub struct Stats {
    /// Strength: damage dealt.
    pub str: i32,
    /// Toughness: damage taken.
    pub tou: i32,
    /// Accuracy: hit chance.
    pub acc: i32,
    /// Evasion: dodge chance.
    pub eva: i32,
    /// Critical chance.
    pub crit: i32,
    /// Composure: critical hits taken.
    pub cool: i32,
    /// Control: stun, disarm and net.
    pub ctl: i32,
    /// Tenacity: resists control.
    pub ten: i32,
    pub spd: i32,
    pub end: i32,
}

impl Stats {
    pub fn values(&self) -> [i32; 10] {
        [self.str, self.tou, self.acc, self.eva, self.crit, self.cool, self.ctl, self.ten, self.spd, self.end]
    }
    pub fn sum(&self) -> i32 {
        self.values().iter().sum()
    }
    pub fn values_mut(&mut self) -> [&mut i32; 10] {
        [
            &mut self.str,
            &mut self.tou,
            &mut self.acc,
            &mut self.eva,
            &mut self.crit,
            &mut self.cool,
            &mut self.ctl,
            &mut self.ten,
            &mut self.spd,
            &mut self.end,
        ]
    }
}

/// When a rule applies.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(tag = "k", content = "v", rename_all = "snake_case")]
pub enum Cond {
    Always,
    FirstTurn,
    SelfHpBelow(u8),
    EnemyHpBelow(u8),
    SelfUnarmed,
    SelfArmed,
    EnemyArmed,
    EnemyUnarmed,
    EnemyStunned,
    EnemyGuarding,
    /// My next plain attack will fill the enemy dodge gauge.
    EnemyDodgeReady,
    /// My next hit will fill my crit gauge.
    CritReady,
    /// The enemy's next hit will fill its crit gauge.
    EnemyCritReady,
    /// My action gauge fills faster than the enemy's.
    Faster,
}

/// Conditions that must be unlocked on an odd level before use.
pub const UNLOCKABLE_CONDS: [Cond; 6] = [
    Cond::EnemyStunned,
    Cond::EnemyGuarding,
    Cond::EnemyDodgeReady,
    Cond::CritReady,
    Cond::EnemyCritReady,
    Cond::Faster,
];

/// What a rule does.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(tag = "k", content = "v", rename_all = "snake_case")]
pub enum Action {
    Attack,
    EquipBest,
    Equip(WeaponKind),
    Throw,
    ThrowNet,
    PowerStrike,
    Disarm,
    Guard,
    Potion,
    Stance(Stance),
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rule {
    pub cond: Cond,
    pub action: Action,
}

/// The choice made when reaching a level. Odd levels give a rule slot or a
/// condition, even levels a node of the skill tree (a weapon or a skill).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum Pick {
    Slot,
    Cond { cond: Cond },
    Weapon { weapon: WeaponKind },
    Skill { skill: Skill },
}

impl Pick {
    pub fn odd(self) -> bool {
        matches!(self, Pick::Slot | Pick::Cond { .. })
    }
}

/// A loadout chosen before a fight: what the fighter carries and how it
/// fights with it. Only carried weapons and skills exist in the fight.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Preset {
    pub name: String,
    pub weapons: Vec<WeaponKind>,
    pub skills: Vec<Skill>,
    pub plan: Vec<Rule>,
    pub stance: Stance,
}

pub const CARRY_WEAPONS: usize = 3;
pub const CARRY_SKILLS: usize = 3;
pub const MAX_PRESETS: usize = 4;

pub const START_POINTS: i32 = 10;
pub const BASE_SLOTS: usize = 3;
pub const MAX_RULES: usize = 8;

/// Stat points to spend at a level: the starting points plus 2 per odd level.
pub fn stat_budget(level: u32) -> i32 {
    START_POINTS + 2 * ((level.max(1) as i32 - 1) / 2)
}

/// Highest value a single stat may reach at a level.
pub fn stat_cap(level: u32) -> i32 {
    4 + level as i32
}

pub fn xp_needed(level: u32) -> u32 {
    2 + level
}

fn one() -> u32 {
    1
}

#[derive(Serialize, Clone, Debug)]
pub struct Fighter {
    pub name: String,
    pub level: u32,
    pub xp: u32,
    pub stats: Stats,
    /// One pick per level gained, in order (level 2 first).
    pub picks: Vec<Pick>,
    /// Owned weapons, derived from the picks (carried ones in a loadout).
    pub weapons: Vec<WeaponKind>,
    /// Owned skills, derived from the picks (carried ones in a loadout).
    pub skills: Vec<Skill>,
    pub presets: Vec<Preset>,
    /// Hits landed with each weapon, over all fights. Kept on respec.
    pub mastery: BTreeMap<WeaponKind, u32>,
    /// Preset used when attacking (the last one chosen).
    pub attack: usize,
    /// Preset used when attacked.
    pub defense: usize,
    /// Plan and stance in play: only set on a loadout, never stored.
    #[serde(skip)]
    pub plan: Vec<Rule>,
    #[serde(skip)]
    pub stance: Stance,
    pub wins: u32,
    pub losses: u32,
}

/// Stored form. Weapons and skills are always rebuilt from the picks, so
/// saves from older versions (with their own stats, weapons and skills) load
/// as fighters whose points and choices are handed back.
#[derive(Deserialize)]
struct FighterSer {
    name: String,
    #[serde(default = "one")]
    level: u32,
    #[serde(default)]
    xp: u32,
    #[serde(default)]
    stats: Stats,
    #[serde(default)]
    picks: Vec<Pick>,
    #[serde(default)]
    presets: Vec<Preset>,
    #[serde(default)]
    mastery: BTreeMap<WeaponKind, u32>,
    #[serde(default)]
    attack: usize,
    #[serde(default)]
    defense: usize,
    /// Before presets: a single plan and stance.
    #[serde(default)]
    plan: Vec<Rule>,
    #[serde(default)]
    stance: Stance,
    #[serde(default)]
    wins: u32,
    #[serde(default)]
    losses: u32,
}

impl<'de> Deserialize<'de> for Fighter {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = FighterSer::deserialize(d)?;
        let mut f = Fighter {
            name: s.name,
            level: s.level.max(1),
            xp: s.xp,
            stats: s.stats,
            picks: s.picks,
            weapons: Vec::new(),
            skills: Vec::new(),
            presets: s.presets,
            mastery: s.mastery,
            attack: s.attack,
            defense: s.defense,
            plan: Vec::new(),
            stance: Stance::Balanced,
            wins: s.wins,
            losses: s.losses,
        };
        f.rebuild();
        if f.presets.is_empty() {
            // The old single plan becomes the first preset, carrying what fits.
            let plan = if s.plan.is_empty() { crate::progress::default_plan() } else { s.plan };
            f.presets.push(Preset { name: "Principal".into(), plan, stance: s.stance, ..Preset::default() });
            f.carry_new(&[], &[]);
        }
        f.clamp_presets();
        Ok(f)
    }
}

impl Fighter {
    pub fn has(&self, s: Skill) -> bool {
        self.skills.contains(&s)
    }

    pub fn max_hp(&self) -> i32 {
        50 + self.stats.end * 6
    }

    /// Recomputes weapons and skills from the picks.
    pub fn rebuild(&mut self) {
        self.weapons.clear();
        self.skills.clear();
        for p in &self.picks {
            match *p {
                Pick::Weapon { weapon } if !self.weapons.contains(&weapon) => self.weapons.push(weapon),
                Pick::Skill { skill } if !self.skills.contains(&skill) => self.skills.push(skill),
                _ => {}
            }
        }
    }

    /// Puts weapons and skills owned now but not in `old_*` into every preset
    /// that still has room, so a new item is carried right away.
    pub fn carry_new(&mut self, old_weapons: &[WeaponKind], old_skills: &[Skill]) {
        let new_w: Vec<_> = self.weapons.iter().copied().filter(|w| !old_weapons.contains(w)).collect();
        let new_s: Vec<_> = self.skills.iter().copied().filter(|s| !old_skills.contains(s)).collect();
        for p in &mut self.presets {
            for w in &new_w {
                if p.weapons.len() < CARRY_WEAPONS && !p.weapons.contains(w) {
                    p.weapons.push(*w);
                }
            }
            for s in &new_s {
                if p.skills.len() < CARRY_SKILLS && !p.skills.contains(s) {
                    p.skills.push(*s);
                }
            }
        }
    }

    /// Drops carried items no longer owned, and keeps indexes in range.
    pub fn clamp_presets(&mut self) {
        if self.presets.is_empty() {
            self.presets.push(Preset { name: "Principal".into(), plan: crate::progress::default_plan(), ..Preset::default() });
        }
        self.presets.truncate(MAX_PRESETS);
        let (weapons, skills) = (self.weapons.clone(), self.skills.clone());
        for p in &mut self.presets {
            let mut seen = Vec::new();
            p.weapons.retain(|w| weapons.contains(w) && !seen.contains(w) && { seen.push(*w); true });
            p.weapons.truncate(CARRY_WEAPONS);
            let mut seen = Vec::new();
            p.skills.retain(|s| skills.contains(s) && !seen.contains(s) && { seen.push(*s); true });
            p.skills.truncate(CARRY_SKILLS);
            p.plan.truncate(MAX_RULES);
        }
        let last = self.presets.len() - 1;
        self.attack = self.attack.min(last);
        self.defense = self.defense.min(last);
    }

    /// The fighter as it enters a fight with preset `i`: only carried
    /// weapons and skills, with the preset's plan and stance.
    pub fn loadout(&self, i: usize) -> Fighter {
        let mut f = self.clone();
        if let Some(p) = self.presets.get(i).or(self.presets.first()) {
            f.weapons = p.weapons.iter().copied().filter(|w| self.weapons.contains(w)).take(CARRY_WEAPONS).collect();
            f.skills = p.skills.iter().copied().filter(|s| self.skills.contains(s)).take(CARRY_SKILLS).collect();
            f.plan = p.plan.clone();
            f.stance = p.stance;
        } else {
            f.weapons.clear();
            f.skills.clear();
        }
        f
    }

    /// The preset a loadout came from, as revealed after a fight.
    pub fn kit(&self, i: usize) -> Preset {
        let l = self.loadout(i);
        let name = self.presets.get(i).or(self.presets.first()).map(|p| p.name.clone()).unwrap_or_default();
        Preset { name, weapons: l.weapons, skills: l.skills, plan: l.plan, stance: l.stance }
    }

    pub fn tier(&self, w: WeaponKind) -> usize {
        mastery_tier(self.mastery.get(&w).copied().unwrap_or(0))
    }

    fn holds(&self, p: Pick) -> bool {
        match p {
            Pick::Weapon { weapon } => self.weapons.contains(&weapon),
            Pick::Skill { skill } => self.skills.contains(&skill),
            _ => false,
        }
    }

    /// Synergies active for this fighter (on a loadout: from carried items).
    pub fn synergies(&self) -> Vec<Synergy> {
        SYNERGIES.iter().filter(|d| d.needs.iter().all(|p| self.holds(*p))).map(|d| d.key).collect()
    }

    /// Rules the plan may use.
    pub fn slots(&self) -> usize {
        (BASE_SLOTS + self.picks.iter().filter(|p| **p == Pick::Slot).count()).min(MAX_RULES)
    }

    /// Whether the condition may be used in this fighter's plan.
    pub fn knows(&self, c: Cond) -> bool {
        !UNLOCKABLE_CONDS.contains(&c) || self.picks.contains(&Pick::Cond { cond: c })
    }

    /// Why the build cannot fight yet, or None when it is complete and valid.
    pub fn build_issue(&self) -> Option<String> {
        let cap = stat_cap(self.level);
        if self.stats.values().iter().any(|v| *v < 0 || *v > cap) {
            return Some(format!("Chaque caractéristique doit rester entre 0 et {cap}."));
        }
        let budget = stat_budget(self.level);
        let sum = self.stats.sum();
        if sum > budget {
            return Some(format!("Trop de points dépensés ({sum} / {budget})."));
        }
        let needed = self.level as usize - 1;
        if self.picks.len() > needed {
            return Some("Trop de choix pour ce niveau.".into());
        }
        let mut slot_picks = 0;
        for (i, p) in self.picks.iter().enumerate() {
            let level = i as u32 + 2;
            if p.odd() != (level % 2 == 1) {
                return Some(format!("Le choix du niveau {level} n'est pas du bon type."));
            }
            match *p {
                Pick::Slot => slot_picks += 1,
                Pick::Cond { cond } if !UNLOCKABLE_CONDS.contains(&cond) => {
                    return Some("Cette condition n'est pas à débloquer.".into());
                }
                Pick::Weapon { weapon: _ } | Pick::Skill { skill: _ } | Pick::Cond { cond: _ } => {
                    if self.picks[..i].contains(p) {
                        return Some(format!("Le choix du niveau {level} est déjà pris."));
                    }
                    if requirement(*p).is_some_and(|r| !self.picks[..i].contains(&r)) {
                        return Some(format!("Le choix du niveau {level} demande d'abord la compétence qui le précède dans l'arbre."));
                    }
                }
            }
        }
        if slot_picks > MAX_RULES - BASE_SLOTS {
            return Some(format!("{MAX_RULES} règles au maximum."));
        }
        if sum < budget {
            return Some(format!("Il reste {} point{} à répartir.", budget - sum, if budget - sum > 1 { "s" } else { "" }));
        }
        if self.picks.len() < needed {
            return Some("Il reste des choix de niveau à faire.".into());
        }
        None
    }

    pub fn complete(&self) -> bool {
        self.build_issue().is_none()
    }
}
