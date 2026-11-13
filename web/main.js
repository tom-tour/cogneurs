'use strict';

// ---------- Texts ----------
const WEAPONS = {
  knife: ['Couteau', 'Rapide, critiques fréquents.'],
  sword: ['Épée', 'Solide et précise.'],
  club: ['Massue', 'Lente mais très forte, peut sonner (jauge de 25 par coup).'],
  spear: ['Lance', 'Allonge : ne peut pas être contrée.'],
  flail: ['Fléau', 'Traverse la garde et le bouclier.'],
  net: ['Filet', "À lancer une fois : immobilise l'adversaire un tour."],
};
const SKILLS = {
  power_strike: ['Coup puissant', 'Dégâts ×2, recharge de 3 tours.'],
  shield: ['Bouclier', 'Jauge de blocage de 12 par coup, et « Se protéger » bloque 75 % au lieu de 50 %.'],
  strong_guard: ['Garde renforcée', 'Quand tu te protèges, le fléau ne traverse plus ta garde.'],
  brace: ['Encaisser', 'Les coups puissants reçus font ×1,5 au lieu de ×2.'],
  precise_strikes: ['Coups précis', 'Tes critiques font ×2 au lieu de ×1,5.'],
  ice_blood: ['Sang glacé', 'Les critiques reçus font 0,25 de moins (×1,25 au lieu de ×1,5).'],
  counter: ['Riposte', 'Jauge de riposte : +40 par esquive, +60 par coup paré en garde.'],
  sharp_riposte: ['Riposte affûtée', 'Tes ripostes font 100 % des dégâts au lieu de 60 %.'],
  disarm: ['Désarmement', "Fait tomber l'arme adverse (perdue pour le combat). Recharge 3 tours."],
  resist: ['Résister', 'Compte comme 2 points de Ténacité en plus face au contrôle.'],
  break_free: ['Se libérer', 'Sonné ou pris au filet, tu ne perds que la moitié d’un tour.'],
  grip: ['Poigne', "Désarmé, ton arme tombe à tes pieds : une règle « Prendre » peut la ramasser."],
  fury: ['Furie', '+40 % de dégâts sous 30 % de PV.'],
  bloodlust: ['Frapper blessé', '+20 % de dégâts contre un adversaire sous 50 % de PV.'],
  relentless: ['Acharnement', '+25 % de dégâts contre un adversaire immobilisé.'],
  potion: ['Potion', 'Une potion par combat : rend 40 % des PV.'],
  regen: ['Régénération', '+2 PV au début de chacun de tes tours.'],
  last_stand: ['Tenir bon', '-25 % de dégâts reçus sous 30 % de PV.'],
};
// Skill tree branches, in opposed pairs (offense, then the defense that answers it).
const BRANCHES = {
  power: 'Puissance', rampart: 'Rempart',
  finesse: 'Finesse', composure: 'Sang-froid',
  control: 'Contrôle', tenacity: 'Ténacité',
  rage: 'Rage', survival: 'Survie',
};
// Opposed pairs: one attack point cancels exactly one defense point.
const STATS = {
  str: ['Force', "Dégâts infligés : +8 % par point d'écart avec sa Robustesse."],
  tou: ['Robustesse', 'Dégâts reçus : annule la Force adverse point pour point.'],
  acc: ['Précision', "Chance de toucher : +2 % par point d'écart avec son Esquive."],
  eva: ['Esquive', "Chance d'esquiver : annule la Précision adverse point pour point."],
  crit: ['Critique', "Chance de critique : +2 % par point d'écart avec son Sang-froid."],
  cool: ['Sang-froid', 'Critiques reçus : annule le Critique adverse point pour point.'],
  ctl: ['Contrôle', 'Sonner à la massue, désarmer, prendre au filet.'],
  ten: ['Ténacité', 'Résiste au Contrôle adverse point pour point.'],
  spd: ['Vitesse', 'Fréquence des tours.'],
  end: ['Endurance', '+6 PV par point.'],
};
const STAT_KEYS = Object.keys(STATS);
const STAT_PAIRS = [['str', 'tou', '↔'], ['acc', 'eva', '↔'], ['crit', 'cool', '↔'], ['ctl', 'ten', '↔'], ['spd', 'end', '·']];
const MAX_SLOT_PICKS = 5;
const CARRY = 3; // weapons, and skills, a preset may carry
const MAX_PRESETS = 4;
const STANCES = {
  aggressive: ['Agressive', "+20 % de dégâts, -5 % d'esquive"],
  balanced: ['Équilibrée', 'Aucun bonus'],
  defensive: ['Défensive', "-15 % de dégâts, +8 % d'esquive"],
};
// [key, label, takes a threshold, must be unlocked, description]
const CONDS = [
  ['always', 'Toujours'],
  ['first_turn', 'Au premier tour'],
  ['self_hp_below', 'Si mes PV < ', true],
  ['enemy_hp_below', 'Si ses PV < ', true],
  ['self_unarmed', 'Si je suis à mains nues'],
  ['self_armed', "Si j'ai une arme en main"],
  ['enemy_armed', 'Si il est armé'],
  ['enemy_unarmed', 'Si il est à mains nues'],
  ['enemy_stunned', 'Si il est immobilisé', false, true, 'Sonné à la massue ou pris au filet : il va passer son tour.'],
  ['enemy_guarding', 'Si il se protège', false, true, 'Il a choisi « Se protéger » à son dernier tour.'],
  ['enemy_dodge_ready', 'Si sa prochaine esquive est prête', false, true, "Ta prochaine attaque normale remplira sa jauge d'esquive."],
  ['crit_ready', 'Si mon critique est prêt', false, true, 'Ton prochain coup qui touche sera un critique.'],
  ['enemy_crit_ready', 'Si son critique est prêt', false, true, 'Son prochain coup qui touche sera un critique.'],
  ['faster', 'Si je suis plus rapide que lui', false, true, "Ta jauge d'action se remplit plus vite que la sienne (vitesse et arme en main)."],
];
const THRESHOLDS = [25, 40, 50, 75];

// ---------- Engine (Rust → WebAssembly) ----------
let wasm;
async function loadEngine() {
  const bytes = await (await fetch('engine.wasm')).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  wasm = instance.exports;
}
const enc = new TextEncoder();
const dec = new TextDecoder();
function api(fn, args) {
  const req = enc.encode(JSON.stringify({ fn, args }));
  const p = wasm.alloc(req.length);
  new Uint8Array(wasm.memory.buffer, p, req.length).set(req);
  const out = wasm.call(p, req.length);
  const text = dec.decode(new Uint8Array(wasm.memory.buffer, out, wasm.out_len()));
  wasm.dealloc(p, req.length);
  const res = JSON.parse(text);
  if (res.err) throw new Error(res.err);
  return res.ok;
}
const seed = () => Math.floor(Math.random() * 2 ** 32);
const store = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
  del(k) { try { localStorage.removeItem(k); } catch { /* private mode */ } },
};
/** What an opponent shows before the fight: no presets (plans, carried gear, defense). */
const publicView = (f) => ({ ...f, presets: [], attack: 0, defense: 0 });

// ---------- Backends ----------
// Both return the same "view": { fighter, opponents: [{fighter, bot}], history, fights_left }.

/** Offline solo mode: everything runs in the page, saved in localStorage. */
const Local = {
  key: 'cogneurs.local.v2',
  online: false,
  s: null,
  load() {
    try { this.s = JSON.parse(store.get(this.key)); } catch { this.s = null; }
    if (!this.s || !this.s.fighter) return;
    // Older saves are converted: points and choices are handed back.
    this.s.fighter = api('load', { fighter: this.s.fighter });
    if (this.s.opponents.some((o) => !o.stats)) this.reroll_();
  },
  save() { store.set(this.key, JSON.stringify(this.s)); },
  view() {
    const f = this.s.fighter;
    return {
      fighter: f,
      opponents: this.s.opponents.map((o) => ({ fighter: publicView(o), bot: true })),
      history: this.s.history,
      fights_left: null,
    };
  },
  reroll_() { this.s.opponents = api('opponents', { fighter: this.s.fighter, seed: seed() }); },
  async me() { this.load(); return this.s ? this.view() : null; },
  async register(name) {
    this.s = { fighter: api('new_fighter', { name }), opponents: [], history: [] };
    this.reroll_();
    this.save();
    return this.view();
  },
  async savePresets(presets, attack, defense) {
    this.s.fighter = api('set_presets', { fighter: this.s.fighter, presets, attack, defense });
    this.save();
    return this.view();
  },
  async setBuild(stats, picks) {
    this.s.fighter = api('set_build', { fighter: this.s.fighter, stats, picks });
    this.save();
    return this.view();
  },
  async reroll() { this.reroll_(); this.save(); return this.view(); },
  async fight(index, preset) {
    const me = this.s.fighter;
    const issue = info(me).issue;
    if (issue) throw new Error(`Termine d'abord tes améliorations : ${issue}`);
    const foe = this.s.opponents[index];
    const { result, kit, foe_kit } = api('fight', { a: me, b: foe, preset });
    const won = result.winner === 0;
    this.s.fighter = api('record', { fighter: { ...me, attack: preset }, won });
    this.s.history = [{ attacker: me.name, defender: foe.name, won, attacked: true }, ...this.s.history].slice(0, 10);
    this.reroll_();
    this.save();
    return { result, foe: publicView(foe), kit, foe_kit, me: this.view() };
  },
  async leaderboard() { return null; },
  forget() { store.del(this.key); this.s = null; },
};

/** Online mode: the server owns the fighters and runs the fights. */
const Remote = {
  online: true,
  /** Address of the server once found, null while unknown. */
  base: null,
  lastLookup: 0,
  get token() { return store.get('cogneurs.token'); },
  async req(method, path, body) {
    const headers = { 'Content-Type': 'application/json' };
    if (this.token) headers.Authorization = `Bearer ${this.token}`;
    const r = await fetch(`${this.base}/api${path}`, { method, headers, body: body ? JSON.stringify(body) : undefined });
    const data = await r.json().catch(() => ({}));
    if (!r.ok) {
      const e = new Error(data.err || `Erreur serveur (${r.status})`);
      e.status = r.status;
      throw e;
    }
    return data;
  },
  /** Health info of the server at `base`, or null if it does not answer. */
  async check(base) {
    try {
      const r = await fetch(`${base}/api/health`, { signal: AbortSignal.timeout(6000), cache: 'no-store' });
      if (!r.ok) return null;
      const h = await r.json();
      return h.ok === true ? h : null;
    } catch { return null; }
  },
  /**
   * Finds a reachable server: the last known one, the fixed address, then
   * the address published by the home launcher. The published address is
   * read at most every 30 s (the GitHub API allows 60 reads an hour, and
   * unchanged answers revalidated with "no-cache" do not count).
   */
  async locate() {
    if (this.base != null) {
      const h = await this.check(this.base);
      if (h) return h;
    }
    const fixed = window.COGNEURS_API || '';
    if (fixed || !location.hostname.endsWith('github.io')) {
      const h = await this.check(fixed);
      if (h) { this.base = fixed; return h; }
    }
    const discovery = window.COGNEURS_DISCOVERY;
    if (!discovery || Date.now() - this.lastLookup < 30000) return null;
    this.lastLookup = Date.now();
    try {
      const r = await fetch(discovery, { cache: 'no-cache' });
      const data = await r.json();
      const info = data.files ? JSON.parse(Object.values(data.files)[0].content) : data;
      if (info.url) {
        const url = info.url.replace(/\/$/, '');
        const h = await this.check(url);
        if (h) { this.base = url; return h; }
      }
    } catch { /* treated as offline */ }
    return null;
  },
  async me() {
    if (!this.token) return null;
    try { return await this.req('GET', '/me'); } catch (e) {
      if (e.status === 401) { store.del('cogneurs.token'); return null; }
      throw e;
    }
  },
  async register(name) {
    const r = await this.req('POST', '/register', { name });
    store.set('cogneurs.token', r.token);
    return r.me;
  },
  savePresets(presets, attack, defense) { return this.req('PUT', '/me/presets', { presets, attack, defense }); },
  setBuild(stats, picks) { return this.req('PUT', '/me/build', { stats, picks }); },
  reroll() { return this.req('POST', '/me/reroll'); },
  fight(index, preset) { return this.req('POST', '/fight', { index, preset }); },
  leaderboard() { return this.req('GET', '/leaderboard'); },
  forget() { store.del('cogneurs.token'); },
};

let backend = Local;
let view = null;
/** Preset shown in the editor. */
let editing = 0;

// ---------- Helpers ----------
const $ = (s) => document.querySelector(s);
const el = (tag, attrs = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'class') e.className = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) e.setAttribute(k, v);
  }
  for (const k of kids.flat()) if (k != null) e.append(k.nodeType ? k : document.createTextNode(k));
  return e;
};
const maxHp = (f) => 50 + f.stats.end * 6;
const info = (f) => api('info', { fighter: f });
const condDef = (k) => CONDS.find((c) => c[0] === k);
/** Whether the fighter may use this condition (unlocked or always available). */
const knows = (f, k) => { const c = condDef(k); return !c || !c[3] || f.picks.some((p) => p.k === 'cond' && p.cond.k === k); };

function toast(msg) {
  let t = $('#toast');
  if (!t) { t = el('div', { id: 'toast' }); document.body.append(t); }
  t.textContent = msg;
  t.classList.add('show');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => t.classList.remove('show'), 3500);
}
async function guarded(fn) {
  try { return await fn(); } catch (e) { toast(e.message); return undefined; }
}

const pickName = (p) => (p.k === 'weapon' ? WEAPONS[p.weapon][0] : p.k === 'skill' ? SKILLS[p.skill][0] : pickText(p)[0]);

/** The skill tree, two opposed branches per row, with what the draft owns. */
function renderTree(tree, owned) {
  const has = (p) => owned.includes(JSON.stringify(p));
  const box = $('#build-tree');
  box.replaceChildren();
  for (const b of tree) {
    box.append(el('div', { class: `branch ${b.key}` },
      el('h4', {}, BRANCHES[b.key]),
      b.nodes.map((n) => {
        const state = has(n.pick) ? 'own' : !n.req || has(n.req) ? 'open' : 'locked';
        const [, desc] = pickText(n.pick);
        return el('div', { class: `node ${state}${n.req ? ' child' : ''}`, title: desc },
          el('b', {}, pickName(n.pick), n.pick.k === 'weapon' ? el('small', {}, ' arme') : null),
          el('span', {}, desc));
      })));
  }
}

function pickText(p) {
  if (p.k === 'slot') return ['+1 place de règle', `Ton plan peut contenir une règle de plus (${3 + MAX_SLOT_PICKS} au maximum).`];
  if (p.k === 'cond') { const c = condDef(p.cond.k); return [`Condition : ${c[1]}`, c[4]]; }
  if (p.k === 'weapon') return [`Arme : ${WEAPONS[p.weapon][0]}`, WEAPONS[p.weapon][1]];
  return [`Compétence : ${SKILLS[p.skill][0]}`, SKILLS[p.skill][1]];
}

function actionChoices(f) {
  const out = [['attack', 'Attaquer'], ['guard', 'Se protéger'], ['equip_best', 'Prendre ma meilleure arme']];
  const held = f.weapons.filter((w) => w !== 'net');
  if (held.length) {
    held.forEach((w) => out.push([`equip:${w}`, `Prendre : ${WEAPONS[w][0]}`]));
    out.push(['throw', 'Lancer mon arme (×1,4, perdue)']);
  }
  if (f.weapons.includes('net')) out.push(['throw_net', 'Lancer le filet']);
  if (f.skills.includes('power_strike')) out.push(['power_strike', 'Coup puissant']);
  if (f.skills.includes('disarm')) out.push(['disarm', 'Désarmer']);
  if (f.skills.includes('potion')) out.push(['potion', 'Boire la potion']);
  Object.keys(STANCES).forEach((s) => out.push([`stance:${s}`, `Posture : ${STANCES[s][0]}`]));
  return out;
}
const actionKey = (a) => (a.v ? `${a.k}:${a.v}` : a.k);
const keyToAction = (key) => {
  const [k, v] = key.split(':');
  return v ? { k, v } : { k };
};
function actionText(a) {
  if (a.k === 'equip') return `Prendre : ${WEAPONS[a.v][0]}`;
  if (a.k === 'stance') return `Posture : ${STANCES[a.v][0]}`;
  return ({ attack: 'Attaquer', guard: 'Se protéger', equip_best: 'Prendre sa meilleure arme', throw: 'Lancer son arme', throw_net: 'Lancer le filet', power_strike: 'Coup puissant', disarm: 'Désarmer', potion: 'Boire la potion' })[a.k] || a.k;
}
function ruleText(r) {
  const c = CONDS.find((x) => x[0] === r.cond.k);
  const cond = c ? c[1] + (c[2] ? `${r.cond.v} %` : '') : r.cond.k;
  return `${cond} → ${actionText(r.action)}`;
}

// ---------- Screens ----------
function show(id) {
  for (const s of document.querySelectorAll('.screen')) s.hidden = s.id !== id;
  $('#reset').hidden = id !== 'home';
  $('#to-menu').hidden = id === 'menu';
  $('#mode').hidden = id === 'menu' || !backend.online;
  if (id === 'menu') { $('#build').hidden = true; renderMenu(); startMenuArt(); } else stopMenuArt();
}

function setView(v) {
  if (!v) return;
  view = v;
  renderAll();
}

function renderAll() {
  const f = view.fighter;
  renderSheet(f);
  renderPlan(f);
  renderOpponents();
  renderHistory();
  if (info(f).issue && buildDismissed !== `${f.name}/${f.level}`) openBuild();
}

function chipList(f) {
  return el('div', { class: 'chips' },
    f.weapons.length ? f.weapons.map((w) => el('span', { class: 'chip w', title: WEAPONS[w][1] }, WEAPONS[w][0])) : el('span', { class: 'muted' }, 'Aucune arme'),
    f.skills.map((s) => el('span', { class: 'chip s', title: SKILLS[s][1] }, SKILLS[s][0])));
}

function renderSheet(f) {
  const inf = info(f);
  const pairs = el('div', { class: 'sheet-pairs' });
  for (const [a, b, sep] of STAT_PAIRS) {
    pairs.append(
      el('span', { class: 'l', title: STATS[a][1] }, `${STATS[a][0]} `, el('b', {}, f.stats[a])),
      el('span', { class: 'x' }, sep),
      el('span', { title: STATS[b][1] }, el('b', {}, f.stats[b]), ` ${STATS[b][0]}`));
  }
  $('#sheet').replaceChildren(el('div', { class: 'card' },
    el('h2', {}, f.name),
    el('div', { class: 'row between' }, el('span', {}, `Niveau ${f.level}`), el('span', { class: 'muted' }, `${inf.max_hp} PV`)),
    el('div', { class: 'bar xp', title: `${f.xp} / ${inf.xp_needed} XP` }, el('i', { style: `width:${(f.xp / inf.xp_needed) * 100}%` })),
    pairs,
    inf.issue ? el('p', { class: 'warn' }, inf.issue) : null,
    el('button', { class: inf.issue ? '' : 'ghost small', onclick: openBuild }, inf.issue ? 'Faire mes améliorations' : 'Respécialiser'),
    el('h3', {}, 'Inventaire'),
    chipList(f),
    el('p', { class: 'hint' }, `Chaque préréglage emporte au plus ${CARRY} armes et ${CARRY} compétences.`),
    el('p', { class: 'record' }, `${f.wins} victoire${f.wins > 1 ? 's' : ''} · ${f.losses} défaite${f.losses > 1 ? 's' : ''}`),
    el('p', { class: 'hint' }, 'Victoire : +2 XP. Défaite : +1 XP. Niveaux impairs : +2 points et une place de règle ou une condition. Niveaux pairs : une case de l’arbre de compétences.')));
}

let planTimer = null;
/** Saves the presets shortly after the last change. */
function presetsChanged(rerender = true) {
  const f = view.fighter;
  if (rerender) { renderPlan(f); renderOpponents(); }
  clearTimeout(planTimer);
  const b = backend;
  planTimer = setTimeout(() => { planTimer = null; guarded(() => b.savePresets(f.presets, f.attack, f.defense)); }, 400);
}
/** Saves now if a change is still waiting. */
async function flushPresets() {
  if (planTimer == null) return true;
  clearTimeout(planTimer);
  planTimer = null;
  const f = view.fighter;
  return (await guarded(() => backend.savePresets(f.presets, f.attack, f.defense))) !== undefined;
}

function renderPlan(f) {
  editing = Math.min(editing, f.presets.length - 1);
  const p = f.presets[editing];
  const slots = info(f).slots;

  $('#preset-tabs').replaceChildren(
    ...f.presets.map((q, i) => el('button', {
      class: `tab${i === editing ? ' active' : ''}`,
      onclick: () => { editing = i; renderPlan(f); },
    }, q.name,
    i === f.attack ? el('span', { class: 'role', title: 'Utilisé pour attaquer' }, ' ⚔') : null,
    i === f.defense ? el('span', { class: 'role', title: 'Utilisé quand on t’attaque' }, ' 🛡') : null)),
    f.presets.length < MAX_PRESETS
      ? el('button', { class: 'tab add', title: 'Nouveau préréglage, copié de celui affiché', onclick: () => {
        f.presets.push({ ...structuredClone(p), name: `Préréglage ${f.presets.length + 1}` });
        editing = f.presets.length - 1;
        presetsChanged();
      } }, '+ Nouveau')
      : null);

  const name = $('#preset-name');
  name.value = p.name;
  name.oninput = () => { p.name = name.value; presetsChanged(false); };
  name.onchange = () => renderPlan(f);
  const role = (id, key) => {
    const box = $(id);
    box.checked = f[key] === editing;
    box.disabled = box.checked;
    box.onchange = () => { f[key] = editing; presetsChanged(); };
  };
  role('#preset-attack', 'attack');
  role('#preset-defense', 'defense');
  $('#preset-delete').disabled = f.presets.length < 2;
  $('#preset-delete').onclick = () => {
    if (!confirm(`Supprimer le préréglage « ${p.name} » ?`)) return;
    f.presets.splice(editing, 1);
    for (const key of ['attack', 'defense']) {
      if (f[key] === editing) f[key] = 0;
      else if (f[key] > editing) f[key] -= 1;
    }
    editing = Math.max(0, editing - 1);
    presetsChanged();
  };

  // Carried gear: a toggle per owned item, up to the limit.
  const toggles = (owned, carried, names, cls) => owned.map((x) => {
    const on = carried.includes(x);
    return el('button', {
      class: `chip ${cls} toggle${on ? ' on' : ''}`, title: names[x][1], 'aria-pressed': on,
      disabled: !on && carried.length >= CARRY,
      onclick: () => {
        if (on) carried.splice(carried.indexOf(x), 1); else carried.push(x);
        presetsChanged();
      },
    }, on ? '✓ ' : '', names[x][0]);
  });
  $('#carry').replaceChildren(
    el('div', { class: 'carry-row' }, el('span', { class: 'muted' }, `Armes ${p.weapons.length}/${CARRY}`),
      f.weapons.length ? toggles(f.weapons, p.weapons, WEAPONS, 'w') : el('span', { class: 'muted' }, 'aucune pour l’instant')),
    el('div', { class: 'carry-row' }, el('span', { class: 'muted' }, `Compétences ${p.skills.length}/${CARRY}`),
      f.skills.length ? toggles(f.skills, p.skills, SKILLS, 's') : el('span', { class: 'muted' }, 'aucune pour l’instant')));

  const stanceSel = $('#stance');
  stanceSel.replaceChildren(...Object.entries(STANCES).map(([k, [n, d]]) => el('option', { value: k, selected: p.stance === k }, `${n} (${d})`)));
  stanceSel.onchange = () => { p.stance = stanceSel.value; presetsChanged(); };

  const choices = actionChoices(p);
  const list = $('#plan');
  list.replaceChildren();
  p.plan.forEach((r, i) => {
    const usable = CONDS.filter(([k]) => knows(f, k) || k === r.cond.k);
    const condSel = el('select', {}, usable.map(([k, n]) => el('option', { value: k, selected: r.cond.k === k }, knows(f, k) ? n : `🔒 ${n}`)));
    const def = condDef(r.cond.k);
    const thrSel = def && def[2]
      ? el('select', {}, THRESHOLDS.map((t) => el('option', { value: t, selected: r.cond.v === t }, `${t} %`)))
      : null;
    const carried = choices.some(([k]) => k === actionKey(r.action));
    const opts = carried ? choices : [...choices, [actionKey(r.action), `⛔ ${actionText(r.action)} (pas emporté)`]];
    const actSel = el('select', {}, opts.map(([k, n]) => el('option', { value: k, selected: actionKey(r.action) === k }, n)));
    condSel.onchange = () => {
      const d = CONDS.find((c) => c[0] === condSel.value);
      r.cond = d[2] ? { k: d[0], v: 50 } : { k: d[0] };
      presetsChanged();
    };
    if (thrSel) thrSel.onchange = () => { r.cond.v = +thrSel.value; presetsChanged(); };
    actSel.onchange = () => { r.action = keyToAction(actSel.value); presetsChanged(); };
    const move = (d) => () => {
      const j = i + d;
      if (j < 0 || j >= p.plan.length) return;
      [p.plan[i], p.plan[j]] = [p.plan[j], p.plan[i]];
      presetsChanged();
    };
    const why = !knows(f, r.cond.k) ? 'Condition à débloquer : cette règle est ignorée.'
      : !carried ? 'Ce préréglage n’emporte pas ce qu’il faut : cette règle est ignorée.' : null;
    const off = i >= slots || why;
    list.append(el('li', { class: off ? 'off' : '', title: why }, condSel, thrSel, el('span', { class: 'arrow' }, '→'), actSel,
      el('span', { class: 'tools' },
        el('button', { class: 'ghost small', title: 'Monter', onclick: move(-1), disabled: i === 0 }, '▲'),
        el('button', { class: 'ghost small', title: 'Descendre', onclick: move(1), disabled: i === p.plan.length - 1 }, '▼'),
        el('button', { class: 'ghost small', title: 'Supprimer', onclick: () => { p.plan.splice(i, 1); presetsChanged(); } }, '✕'))));
  });
  $('#add-rule').disabled = p.plan.length >= slots;
  $('#add-rule').onclick = () => { p.plan.push({ cond: { k: 'always' }, action: { k: 'attack' } }); presetsChanged(); };
  $('#slots').textContent = `${Math.min(p.plan.length, slots)} / ${slots} règles utilisées${p.plan.length > slots ? ' (les règles grisées sont ignorées)' : ''}.`;
  // A weapon carried but never drawn is a common trap now that nothing is automatic.
  const active = p.plan.slice(0, slots).filter((r) => knows(f, r.cond.k));
  const draws = active.some((r) => r.action.k === 'equip_best' || (r.action.k === 'equip' && p.weapons.includes(r.action.v)));
  const warn = $('#plan-warn');
  warn.hidden = draws || !p.weapons.some((w) => w !== 'net');
  warn.textContent = 'Ce préréglage emporte une arme, mais aucune règle ne la prend : ton cogneur se battra à mains nues.';
}

function renderOpponents() {
  const f = view.fighter;
  const box = $('#opponents');
  box.replaceChildren();
  const left = view.fights_left;
  $('#fights-left').textContent = left == null ? '' : `Combats restants aujourd'hui : ${left}`;
  const sel = $('#attack-preset');
  sel.replaceChildren(...f.presets.map((q, i) => el('option', { value: i, selected: i === f.attack }, q.name)));
  sel.onchange = () => { f.attack = +sel.value; presetsChanged(); };
  $('#defense-note').textContent = `Quand on t'attaque, tu te défends avec « ${f.presets[f.defense].name} ».`;
  view.opponents.forEach(({ fighter: o, bot }, i) => {
    box.append(el('div', { class: 'opp' },
      el('div', { class: 'row between' },
        el('h3', {}, o.name, backend.online ? el('span', { class: 'tag', style: 'margin-left:6px' }, bot ? 'bot' : 'joueur') : null),
        el('span', { class: 'muted' }, `Niv. ${o.level} · ${maxHp(o)} PV`)),
      el('div', { class: 'mini' }, ...STAT_KEYS.map((k) => el('span', { title: STATS[k][0] }, `${STATS[k][0].slice(0, 3)} `, el('b', {}, o.stats[k])))),
      el('div', { class: 'muted', style: 'font-size:12px' }, `Inventaire (il en emporte au plus ${CARRY} + ${CARRY}) :`),
      chipList(o),
      el('div', { class: 'actions' },
        el('button', { onclick: () => startFight(i), disabled: left === 0 }, 'Combattre'))));
  });
}

async function renderHistory() {
  const h = view.history || [];
  $('#history-card').hidden = !h.length;
  $('#history').replaceChildren(...h.map((x) => el('li', { class: x.won ? 'won' : 'lost' },
    x.attacked ? `${x.won ? 'Victoire' : 'Défaite'} contre ${x.defender}` : `${x.attacker} t'a attaqué : ${x.won ? 'défendu' : 'perdu'}`)));
  const board = await guarded(() => backend.leaderboard());
  $('#board-card').hidden = !board;
  if (board) {
    $('#board').replaceChildren(...board.map((r) => el('li', { class: r.name === view.fighter.name ? 'me' : '' },
      el('b', {}, r.name), ` · niv. ${r.level} · ${r.wins} V / ${r.losses} D`)));
  }
}

// ---------- Build editor (creation, level-ups, respec) ----------
let buildDismissed = null;

function openBuild() {
  const f = view.fighter;
  const inf = info(f);
  const draft = { stats: { ...f.stats }, picks: f.picks.map((p) => ({ ...p })) };
  while (draft.picks.length < f.level - 1) draft.picks.push(null);
  const key = (x) => (x ? JSON.stringify(x) : '');
  // The engine takes the choices in level order, up to the first one missing.
  const chosen = () => { const i = draft.picks.indexOf(null); return i < 0 ? draft.picks : draft.picks.slice(0, i); };
  const fresh = f.level === 1 && !f.wins && !f.losses;
  $('#build-title').textContent = fresh ? `Crée ${f.name} : répartis tes points` : inf.issue ? `Niveau ${f.level} : améliorations` : 'Respécialisation';
  $('#build-levels-box').hidden = f.level < 2;

  const render = () => {
    const spent = STAT_KEYS.reduce((n, k) => n + draft.stats[k], 0);
    const left = inf.budget - spent;
    $('#build-points').textContent = `· ${left} point${left > 1 ? 's' : ''} à répartir (${inf.budget} au total, ${inf.cap} max par caractéristique)`;
    $('#build-stats').replaceChildren(...STAT_KEYS.map((k) => {
      const v = draft.stats[k];
      const step = (d) => () => { draft.stats[k] = v + d; render(); };
      return el('div', { class: 'build-stat' },
        el('span', { class: 'name', title: STATS[k][1] }, STATS[k][0], el('small', {}, STATS[k][1])),
        el('button', { class: 'ghost small', onclick: step(-1), disabled: v <= 0, 'aria-label': `Retirer un point de ${STATS[k][0]}` }, '−'),
        el('b', {}, v),
        el('button', { class: 'ghost small', onclick: step(1), disabled: left <= 0 || v >= inf.cap, 'aria-label': `Ajouter un point à ${STATS[k][0]}` }, '+'));
    }));

    const firstHole = draft.picks.indexOf(null);
    $('#build-levels').replaceChildren(...draft.picks.map((p, i) => {
      const level = i + 2;
      const others = draft.picks.filter((x, j) => x && j !== i);
      const taken = new Set(others.filter((x) => x.k !== 'slot').map(key));
      const slotsTaken = others.filter((x) => x.k === 'slot').length;
      let choices;
      if (level % 2) {
        const opts = inf.odd.filter((x) => !taken.has(key(x)) && !(x.k === 'slot' && slotsTaken >= MAX_SLOT_PICKS));
        choices = opts.map((x) => el('option', { value: key(x), selected: key(x) === key(p) }, pickText(x)[0]));
      } else {
        // Tree nodes, grouped by branch; a node needs its prerequisite at an earlier level.
        const before = new Set(draft.picks.slice(0, i).filter(Boolean).map(key));
        choices = inf.tree.map((b) => el('optgroup', { label: BRANCHES[b.key] },
          b.nodes.filter((n) => !taken.has(key(n.pick))).map((n) => {
            const locked = n.req && !before.has(key(n.req));
            return el('option', { value: key(n.pick), selected: key(n.pick) === key(p), disabled: locked && key(n.pick) !== key(p) },
              locked ? `🔒 ${pickText(n.pick)[0]} (après ${pickName(n.req)})` : pickText(n.pick)[0]);
          })));
      }
      const sel = el('select', { disabled: firstHole >= 0 && i > firstHole },
        el('option', { value: '' }, '— à choisir —'), choices);
      sel.onchange = () => { draft.picks[i] = sel.value ? JSON.parse(sel.value) : null; render(); };
      return el('li', {}, el('span', { class: 'lv' }, `Niveau ${level}`), sel, el('span', { class: 'desc' }, p ? pickText(p)[1] : ''));
    }));

    renderTree(inf.tree, draft.picks.filter(Boolean).map(key));

    let issue;
    try { issue = info(api('set_build', { fighter: f, stats: draft.stats, picks: chosen() })).issue; } catch (e) { issue = e.message; }
    $('#build-issue').textContent = issue || '';
    $('#build-save').textContent = issue ? 'Enregistrer (incomplet)' : 'Valider';
  };

  $('#build-save').onclick = async () => {
    const v = await guarded(() => backend.setBuild(draft.stats, chosen()));
    if (!v) return;
    buildDismissed = `${f.name}/${f.level}`;
    $('#build').hidden = true;
    setView(v);
  };
  $('#build-cancel').onclick = () => { buildDismissed = `${f.name}/${f.level}`; $('#build').hidden = true; };
  render();
  $('#build').hidden = false;
}

// ---------- Fight replay ----------
let anim = null;

async function startFight(i) {
  const f = view.fighter;
  if (info(f).issue) { openBuild(); return; }
  if (!(await flushPresets())) return;
  const r = await guarded(() => backend.fight(i, f.attack));
  if (!r) return;
  view = r.me;
  playFight(f, r.kit, r.foe, r.foe_kit, r.result);
}

function logLine(ev, names) {
  const a = names[ev.who];
  const b = names[1 - ev.who];
  const w = ev.weapon ? WEAPONS[ev.weapon][0].toLowerCase() : 'ses poings';
  const dmg = () => `-${ev.value} PV${ev.crit ? ' (critique !)' : ''}${ev.blocked ? ' (paré)' : ''}`;
  switch (ev.kind) {
    case 'attack': return ev.hit ? `${a} frappe ${b} avec ${ev.weapon ? 'son ' + w : w} : ${dmg()}` : `${b} esquive l'attaque de ${a}.`;
    case 'power': return ev.hit ? `${a} assène un coup puissant : ${dmg()}` : `${b} esquive le coup puissant de ${a}.`;
    case 'throw': return ev.hit ? `${a} lance son ${w} sur ${b} : ${dmg()}` : `${a} lance son ${w}… ${b} l'esquive !`;
    case 'counter': return `${a} riposte : -${ev.value} PV`;
    case 'equip': return `${a} prend son ${w}.`;
    case 'net': return ev.hit ? `${a} lance son filet : ${b} est immobilisé !` : `${b} évite le filet de ${a}.`;
    case 'disarm': return ev.hit ? `${a} désarme ${b} : ${w} perdu !` : `${a} tente de désarmer ${b}, qui tient bon.`;
    case 'guard': return `${a} se met en garde.`;
    case 'potion': return `${a} boit une potion : +${ev.value} PV.`;
    case 'stance': return `${a} passe en posture ${STANCES[ev.stance][0].toLowerCase()}.`;
    case 'stunned': return `${a} est sonné !`;
    case 'skip': return `${a} est immobilisé et passe son tour.`;
    case 'end': return `${a} remporte le combat !`;
    default: return ev.kind;
  }
}

const TURN_KINDS = new Set(['attack', 'power', 'throw', 'equip', 'net', 'disarm', 'guard', 'potion', 'stance', 'skip']);
const STRIKES = new Set(['attack', 'power', 'throw', 'counter']);

function playFight(meF, kit, foeF, foeKit, res) {
  const names = [meF.name, foeF.name];
  const plans = [kit.plan, foeKit.plan];
  const kits = [kit, foeKit];
  const canvas = $('#stage');
  const ctx = canvas.getContext('2d');
  const sides = [meF, foeF].map((f, i) => ({
    name: f.name, max: res.max_hp[i], hp: res.max_hp[i], shown: res.max_hp[i], dodge: 0, crit: 0,
    wield: null, guard: false, stunned: false, stance: kits[i].stance,
    home: i === 0 ? 220 : 500, dir: i === 0 ? 1 : -1, color: i === 0 ? '#e8893a' : '#6ea8e0', flash: 0,
  }));
  const floats = [];
  const projectiles = [];
  const log = $('#log');
  log.replaceChildren();
  $('#result').hidden = true;
  $('#fight-close').hidden = true;
  $('#fight').hidden = false;

  let idx = -1;
  let t = 1;
  let speed = +(store.get('cogneurs.speed') || 1) || 1;
  const setSpeedButtons = () => document.querySelectorAll('.speed').forEach((b) => b.classList.toggle('active', +b.dataset.speed === speed));
  setSpeedButtons();
  document.querySelectorAll('.speed').forEach((b) => { b.onclick = () => { speed = +b.dataset.speed; store.set('cogneurs.speed', speed); setSpeedButtons(); }; });

  function apply(ev) {
    const me = sides[ev.who];
    const other = sides[1 - ev.who];
    if (TURN_KINDS.has(ev.kind)) me.guard = false;
    const line = el('li', { class: ev.who === 0 ? 'p' : 'e' });
    const rule = ev.rule >= 0 ? plans[ev.who][ev.rule] : null;
    if (rule) line.append(el('span', { class: 'r', title: ruleText(rule) }, `R${ev.rule + 1}`));
    line.append(logLine(ev, names));
    log.append(line);
    log.scrollTop = log.scrollHeight;
    const say = (s, text, color, big) => floats.push({ x: s.home, y: 120, text, color, big, life: 1 });
    switch (ev.kind) {
      case 'attack': case 'power': case 'counter':
        if (ev.hit) { other.flash = 1; say(other, `-${ev.value}`, ev.crit ? '#ffd34d' : '#ff6b5b', ev.crit || ev.kind === 'power'); }
        else say(other, 'esquive', '#a9c6e6');
        if (ev.kind === 'power') say(me, 'Coup puissant', '#f2b54a');
        break;
      case 'throw':
        projectiles.push({ from: me.home, to: other.home, weapon: ev.weapon, life: 1 });
        me.wield = null;
        if (ev.hit) { other.flash = 1; say(other, `-${ev.value}`, '#ff6b5b', ev.crit); } else say(other, 'esquive', '#a9c6e6');
        break;
      case 'equip': me.wield = ev.weapon; say(me, WEAPONS[ev.weapon][0], '#efe4d2'); break;
      case 'net':
        projectiles.push({ from: me.home, to: other.home, weapon: 'net', life: 1 });
        if (ev.hit) { other.stunned = true; say(other, 'Pris au filet !', '#f2b54a'); } else say(other, 'esquive', '#a9c6e6');
        break;
      case 'disarm':
        if (ev.hit) { other.wield = null; say(other, 'Désarmé !', '#f2b54a'); } else say(other, 'tient bon', '#cfc3b0');
        break;
      case 'guard': me.guard = true; say(me, 'Garde', '#a9c6e6'); break;
      case 'potion': say(me, `+${ev.value}`, '#7cc46b', true); break;
      case 'stance': me.stance = ev.stance; say(me, STANCES[ev.stance][0], '#efe4d2'); break;
      case 'stunned': me.stunned = true; say(me, 'Sonné !', '#f2b54a'); break;
      case 'skip': me.stunned = false; break;
      case 'end': finish(); break;
    }
    sides.forEach((s, k) => { s.hp = ev.hp[k]; s.dodge = ev.g[k][0]; s.crit = ev.g[k][1]; });
  }

  function finish() {
    const won = res.winner === 0;
    const r = $('#result');
    r.className = won ? 'win' : 'lose';
    r.replaceChildren(
      el('div', { class: 'big' }, won ? 'Victoire !' : 'Défaite…'),
      el('div', { class: 'muted' }, won ? '+2 XP' : '+1 XP'),
      el('h3', { style: 'margin-top:8px' }, `${foeF.name} révèle son préréglage « ${foeKit.name} »`),
      chipList(foeKit),
      el('div', { class: 'muted', style: 'font-size:12px' }, `Posture ${STANCES[foeKit.stance][0].toLowerCase()}`),
      el('ol', {}, foeKit.plan.map((rule) => el('li', {}, ruleText(rule))), el('li', { class: 'muted' }, 'Sinon → Attaquer')));
    r.hidden = false;
    $('#fight-close').hidden = false;
  }

  $('#skip').onclick = () => {
    while (idx < res.events.length - 1) { idx++; apply(res.events[idx]); }
    t = 1;
    floats.length = 0;
    projectiles.length = 0;
  };
  $('#fight-close').onclick = () => {
    cancelAnimationFrame(anim);
    anim = null;
    $('#fight').hidden = true;
    renderAll();
  };

  let last = performance.now();
  const DUR = 650;
  function frame(now) {
    const dt = Math.min(now - last, 100);
    last = now;
    if (idx < res.events.length - 1 || t < 1) {
      t += (dt * speed) / DUR;
      if (t >= 1 && idx < res.events.length - 1) {
        t = 0;
        idx++;
        apply(res.events[idx]);
      }
    }
    for (const s of sides) {
      s.shown += (s.hp - s.shown) * Math.min(1, dt / 120);
      s.flash = Math.max(0, s.flash - dt / 300);
    }
    for (const f of floats) { f.life -= dt / 1100; f.y -= dt * 0.03; }
    for (const p of projectiles) p.life -= (dt * speed) / 450;
    while (floats.length && floats[0].life <= 0) floats.shift();
    while (projectiles.length && projectiles[0].life <= 0) projectiles.shift();
    draw(ctx, sides, floats, projectiles, idx >= 0 ? res.events[idx] : null, Math.min(t, 1));
    anim = requestAnimationFrame(frame);
  }
  cancelAnimationFrame(anim);
  anim = requestAnimationFrame(frame);
}

// ---------- Server status ----------
const Status = {
  state: 'searching', // 'searching' | 'online' | 'offline'
  players: null,
  me: undefined, // online fighter summary, fetched once the server is found
  async refresh() {
    const h = await Remote.locate();
    const was = this.state;
    this.state = h ? 'online' : 'offline';
    this.players = h ? h.players : null;
    if (h && this.me === undefined && Remote.token) {
      this.me = null;
      try { this.me = (await Remote.req('GET', '/me')).fighter; } catch { this.me = undefined; }
    }
    renderStatus();
    if (was === 'online' && this.state === 'offline' && backend.online && $('#menu').hidden) {
      toast('Le serveur ne répond plus.');
    }
  },
  start() {
    this.refresh();
    setInterval(() => this.refresh(), 10000);
  },
};

function renderStatus() {
  const st = Status.state;
  const label = { searching: 'Recherche du serveur…', online: 'Serveur en ligne', offline: 'Serveur hors ligne' }[st];
  const box = $('#menu-status');
  box.className = `server-status ${st}`;
  box.lastElementChild.textContent = label;
  $('#play-online').disabled = st !== 'online';
  let info = '';
  if (st === 'online') {
    const n = Status.players;
    info = `${n} cogneur${n > 1 ? 's' : ''} inscrit${n > 1 ? 's' : ''}`;
    if (Status.me) info += ` · le tien : ${Status.me.name}, niv. ${Status.me.level}`;
  } else if (st === 'offline') {
    info = Remote.token ? "Ton cogneur en ligne t'attend : il reviendra avec le serveur." : 'Le serveur se met en ligne quand son hôte le lance.';
  }
  $('#online-info').textContent = info;
  // Header badge while playing online.
  const tag = $('#mode');
  tag.className = `tag status ${st}`;
  tag.lastElementChild.textContent = st === 'online' ? 'en ligne' : st === 'offline' ? 'serveur injoignable' : 'connexion…';
}

function renderMenu() {
  Local.load();
  const f = Local.s && Local.s.fighter;
  $('#offline-info').textContent = f ? `Ton cogneur : ${f.name}, niv. ${f.level}` : 'Nouveau cogneur';
  renderStatus();
}

async function enter(b) {
  backend = b;
  view = null;
  const v = await guarded(() => backend.me());
  $('#create-title').textContent = backend.online ? 'Crée ton cogneur (en ligne)' : 'Crée ton cogneur (solo)';
  if (v) { show('home'); setView(v); } else if (v !== undefined) { show('create'); }
}

// ---------- Menu art ----------
let menuAnim = null;
function startMenuArt() {
  if (menuAnim) return;
  const ctx = $('#menu-art').getContext('2d');
  const mk = (i, wield, stance) => ({
    hp: 1, shown: 1, max: 1, dir: i === 0 ? 1 : -1, color: i === 0 ? '#e8893a' : '#6ea8e0',
    flash: 0, wield, guard: false, stunned: false, stance,
  });
  const sides = [mk(0, 'sword', 'aggressive'), mk(1, 'club', 'defensive')];
  const frame = (now) => {
    ctx.clearRect(0, 0, 440, 170);
    const t = now / 1000;
    const clash = Math.max(0, Math.sin(t * 1.6)) ** 6; // a lunge now and then
    sides.forEach((s, i) => {
      ctx.save();
      ctx.translate(0, Math.sin(t * 3 + i * 1.7) * 2.5);
      const x = (i === 0 ? 140 : 300) + s.dir * clash * (i === 0 ? 34 : 4);
      s.flash = i === 1 ? clash * 0.4 : 0;
      drawFighter(ctx, x, 140, s);
      ctx.restore();
    });
    menuAnim = requestAnimationFrame(frame);
  };
  menuAnim = requestAnimationFrame(frame);
}
function stopMenuArt() {
  cancelAnimationFrame(menuAnim);
  menuAnim = null;
}

// ---------- Boot ----------
async function boot() {
  await loadEngine();
  $('#create-form').onsubmit = async (e) => {
    e.preventDefault();
    const name = $('#name').value.trim();
    if (!name) return;
    const v = await guarded(() => backend.register(name));
    if (!v) return;
    show('home');
    setView(v);
  };
  $('#reset').onclick = () => {
    const msg = backend.online
      ? "Quitter ce cogneur ? Il reste dans l'arène, mais tu perds l'accès à ce compte sur ce navigateur."
      : 'Abandonner ce cogneur et en créer un nouveau ?';
    if (!confirm(msg)) return;
    backend.forget();
    if (backend.online) Status.me = undefined;
    view = null;
    $('#name').value = '';
    show('create');
  };
  $('#reroll').onclick = async () => setView(await guarded(() => backend.reroll()));
  $('#to-menu').onclick = () => {
    if (view && backend === Remote) Status.me = view.fighter;
    show('menu');
  };
  $('#play-online').onclick = () => enter(Remote);
  $('#play-offline').onclick = () => enter(Local);

  show('menu');
  Status.start();
}

boot().catch((e) => {
  document.body.append(el('pre', { style: 'color:#e0574a;padding:20px' }, `Erreur de chargement : ${e.message}\nLance le jeu via un serveur local (voir README).`));
});
