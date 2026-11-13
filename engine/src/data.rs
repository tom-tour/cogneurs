//! Game data: fighters, weapons, skills and battle plans.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
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
    /// Passive 12% block chance; the Guard action blocks 75% instead of 50%.
    Shield,
    /// One potion per fight, heals 40% of max HP.
    Potion,
    /// Double damage strike, 3 turns cooldown.
    PowerStrike,
    /// Knock the enemy weapon away (it is lost for the fight).
    Disarm,
    /// Riposte after a dodge or a guarded hit.
    Counter,
    /// +40% damage under 30% HP.
    Fury,
    /// -2 damage on every hit taken.
    Toughness,
    /// +10% dodge.
    Evasion,
}

pub const ALL_SKILLS: [Skill; 8] = [
    Skill::Shield,
    Skill::Potion,
    Skill::PowerStrike,
    Skill::Disarm,
    Skill::Counter,
    Skill::Fury,
    Skill::Toughness,
    Skill::Evasion,
];

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Aggressive,
    #[default]
    Balanced,
    Defensive,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Str,
    Agi,
    Spd,
    End,
}

pub const ALL_STATS: [Stat; 4] = [Stat::Str, Stat::Agi, Stat::Spd, Stat::End];

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
}

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

fn one() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Fighter {
    pub name: String,
    #[serde(default = "one")]
    pub level: u32,
    #[serde(default)]
    pub xp: u32,
    #[serde(default)]
    pub pending: u32,
    pub str: i32,
    pub agi: i32,
    pub spd: i32,
    pub end: i32,
    #[serde(default)]
    pub weapons: Vec<WeaponKind>,
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub plan: Vec<Rule>,
    #[serde(default)]
    pub stance: Stance,
    #[serde(default)]
    pub wins: u32,
    #[serde(default)]
    pub losses: u32,
}

impl Fighter {
    pub fn has(&self, s: Skill) -> bool {
        self.skills.contains(&s)
    }

    pub fn max_hp(&self) -> i32 {
        30 + self.end * 8
    }

    pub fn stat_mut(&mut self, s: Stat) -> &mut i32 {
        match s {
            Stat::Str => &mut self.str,
            Stat::Agi => &mut self.agi,
            Stat::Spd => &mut self.spd,
            Stat::End => &mut self.end,
        }
    }
}

pub const MAX_RULES: usize = 8;

/// Number of rules a fighter may use in its plan.
pub fn plan_slots(level: u32) -> usize {
    (2 + level as usize / 2).min(MAX_RULES)
}

pub fn xp_needed(level: u32) -> u32 {
    2 + level
}
