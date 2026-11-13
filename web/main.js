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
  shield: ['Bouclier', 'Jauge de blocage de 12 par coup, et « Se protéger » bloque 75 % au lieu de 50 %.'],
  potion: ['Potion', 'Une potion par combat : rend 40 % des PV.'],
  power_strike: ['Coup puissant', 'Dégâts ×2, recharge de 3 tours.'],
  disarm: ['Désarmement', "Fait tomber l'arme adverse (perdue pour le combat). Recharge 3 tours."],
  counter: ['Riposte', 'Jauge de riposte : +40 par esquive, +60 par coup paré en garde.'],
  fury: ['Furie', '+40 % de dégâts sous 30 % de PV.'],
  toughness: ['Cuir épais', '-2 dégâts sur chaque coup reçu.'],
  evasion: ['Esquive', '+10 % d\'esquive.'],
};
const STATS = {
  str: ['Force', 'Dégâts : +8 % par point.'],
  agi: ['Agilité', 'Précision, esquive, critiques, désarmement.'],
  spd: ['Vitesse', 'Fréquence des tours.'],
  end: ['Endurance', '+8 PV par point.'],
};
const STANCES = {
  aggressive: ['Agressive', "+20 % de dégâts, -5 % d'esquive"],
  balanced: ['Équilibrée', 'Aucun bonus'],
  defensive: ['Défensive', "-15 % de dégâts, +8 % d'esquive"],
};
const CONDS = [
  ['always', 'Toujours'],
  ['first_turn', 'Au premier tour'],
  ['self_hp_below', 'Si mes PV < ', true],
  ['enemy_hp_below', 'Si ses PV < ', true],
  ['self_unarmed', 'Si je suis à mains nues'],
  ['self_armed', "Si j'ai une arme en main"],
  ['enemy_armed', 'Si il est armé'],
  ['enemy_unarmed', 'Si il est à mains nues'],
  ['enemy_stunned', 'Si il est immobilisé'],
  ['enemy_guarding', 'Si il se protège'],
  ['enemy_dodge_ready', 'Si sa prochaine esquive est prête'],
  ['crit_ready', 'Si mon critique est prêt'],
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
const stripPlan = (f) => ({ ...f, plan: [] });

// ---------- Backends ----------
// Both return the same "view": { fighter, options, opponents: [{fighter, bot}], history, fights_left }.

/** Offline solo mode: everything runs in the page, saved in localStorage. */
const Local = {
  key: 'cogneurs.local.v2',
  online: false,
  s: null,
  load() { try { this.s = JSON.parse(store.get(this.key)); } catch { this.s = null; } },
  save() { store.set(this.key, JSON.stringify(this.s)); },
  view() {
    const f = this.s.fighter;
    return {
      fighter: f,
      options: f.pending > 0 ? api('options', { fighter: f }) : [],
      opponents: this.s.opponents.map((o) => ({ fighter: stripPlan(o), bot: true })),
      history: this.s.history,
      fights_left: null,
    };
  },
  reroll_() { this.s.opponents = api('opponents', { fighter: this.s.fighter, seed: seed() }); },
  async me() { this.load(); return this.s ? this.view() : null; },
  async register(name) {
    this.s = { fighter: api('new_fighter', { name, seed: seed() }), opponents: [], history: [] };
    this.reroll_();
    this.save();
    return this.view();
  },
  async savePlan(plan, stance) {
    Object.assign(this.s.fighter, { plan, stance });
    this.save();
    return this.view();
  },
  async level(choice) {
    const u = api('options', { fighter: this.s.fighter })[choice];
    this.s.fighter = api('apply', { fighter: this.s.fighter, upgrade: u });
    this.save();
    return this.view();
  },
  async reroll() { this.reroll_(); this.save(); return this.view(); },
  async fight(index) {
    const me = this.s.fighter;
    const foe = this.s.opponents[index];
    const result = api('fight', { a: me, b: foe });
    const won = result.winner === 0;
    this.s.fighter = api('record', { fighter: me, won });
    this.s.history = [{ attacker: me.name, defender: foe.name, won, attacked: true }, ...this.s.history].slice(0, 10);
    this.reroll_();
    this.save();
    return { result, foe, me: this.view() };
  },
  async leaderboard() { return null; },
  forget() { store.del(this.key); this.s = null; },
};

/** Online mode: the server owns the fighters and runs the fights. */
const Remote = {
  online: true,
  base: window.COGNEURS_API || '',
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
  async available() {
    try {
      const r = await fetch(`${this.base}/api/health`, { signal: AbortSignal.timeout(4000) });
      return r.ok && (await r.json()).ok === true;
    } catch { return false; }
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
  savePlan(plan, stance) { return this.req('PUT', '/me/plan', { plan, stance }); },
  level(choice) { return this.req('POST', '/me/level', { choice }); },
  reroll() { return this.req('POST', '/me/reroll'); },
  fight(index) { return this.req('POST', '/fight', { index }); },
  leaderboard() { return this.req('GET', '/leaderboard'); },
  forget() { store.del('cogneurs.token'); },
};

let backend = Local;
let view = null;
let preds = {};

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
const maxHp = (f) => 30 + f.end * 8;

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

function upgradeText(u) {
  if (u.k === 'stat') return [`+${u.amount} ${STATS[u.stat][0]}`, STATS[u.stat][1]];
  if (u.k === 'weapon') return [`Arme : ${WEAPONS[u.weapon][0]}`, WEAPONS[u.weapon][1]];
  return [`Compétence : ${SKILLS[u.skill][0]}`, SKILLS[u.skill][1]];
}

function actionChoices(f) {
  const out = [['attack', 'Attaquer'], ['guard', 'Se protéger']];
  const held = f.weapons.filter((w) => w !== 'net');
  if (held.length) {
    out.push(['equip_best', 'Prendre ma meilleure arme']);
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
function ruleText(r) {
  const c = CONDS.find((x) => x[0] === r.cond.k);
  const cond = c ? c[1] + (c[2] ? `${r.cond.v} %` : '') : r.cond.k;
  const a = r.action;
  let act;
  if (a.k === 'equip') act = `Prendre : ${WEAPONS[a.v][0]}`;
  else if (a.k === 'stance') act = `Posture : ${STANCES[a.v][0]}`;
  else act = ({ attack: 'Attaquer', guard: 'Se protéger', equip_best: 'Prendre sa meilleure arme', throw: 'Lancer son arme', throw_net: 'Lancer le filet', power_strike: 'Coup puissant', disarm: 'Désarmer', potion: 'Boire la potion' })[a.k] || a.k;
  return `${cond} → ${act}`;
}

// ---------- Screens ----------
function show(id) {
  for (const s of document.querySelectorAll('.screen')) s.hidden = s.id !== id;
  $('#reset').hidden = id !== 'home';
}

function setView(v) {
  if (!v) return;
  view = v;
  preds = {};
  renderAll();
}

function renderAll() {
  const f = view.fighter;
  renderSheet(f);
  renderPlan(f);
  renderOpponents();
  renderHistory();
  if (view.options.length) openLevelUp();
}

function chipList(f) {
  return el('div', { class: 'chips' },
    f.weapons.length ? f.weapons.map((w) => el('span', { class: 'chip w', title: WEAPONS[w][1] }, WEAPONS[w][0])) : el('span', { class: 'muted' }, 'Aucune arme'),
    f.skills.map((s) => el('span', { class: 'chip s', title: SKILLS[s][1] }, SKILLS[s][0])));
}

function renderSheet(f) {
  const meta = api('meta', { level: f.level });
  const top = Math.max(10, f.str, f.agi, f.spd, f.end);
  const stats = el('div', { class: 'stats' });
  for (const k of Object.keys(STATS)) {
    stats.append(el('span', { title: STATS[k][1] }, STATS[k][0]),
      el('div', { class: 'bar' }, el('i', { style: `width:${(f[k] / top) * 100}%` })), el('b', {}, f[k]));
  }
  $('#sheet').replaceChildren(el('div', { class: 'card' },
    el('h2', {}, f.name),
    el('div', { class: 'row between' }, el('span', {}, `Niveau ${f.level}`), el('span', { class: 'muted' }, `${maxHp(f)} PV`)),
    el('div', { class: 'bar xp', title: `${f.xp} / ${meta.xp_needed} XP` }, el('i', { style: `width:${(f.xp / meta.xp_needed) * 100}%` })),
    stats,
    el('h3', {}, 'Équipement'),
    chipList(f),
    el('p', { class: 'record' }, `${f.wins} victoire${f.wins > 1 ? 's' : ''} · ${f.losses} défaite${f.losses > 1 ? 's' : ''}`),
    el('p', { class: 'hint' }, 'Victoire : +2 XP. Défaite : +1 XP. À chaque niveau tu choisis 1 amélioration parmi 3, et tous les 2 niveaux ton plan gagne une règle.')));
}

let planTimer = null;
function planChanged() {
  preds = {};
  renderPlan(view.fighter);
  renderOpponents();
  clearTimeout(planTimer);
  const { plan, stance } = view.fighter;
  planTimer = setTimeout(() => guarded(() => backend.savePlan(plan, stance)), 400);
}

function renderPlan(f) {
  const slots = api('meta', { level: f.level }).slots;
  const stanceSel = $('#stance');
  stanceSel.replaceChildren(...Object.entries(STANCES).map(([k, [n, d]]) => el('option', { value: k, selected: f.stance === k }, `${n} (${d})`)));
  stanceSel.onchange = () => { f.stance = stanceSel.value; planChanged(); };

  const choices = actionChoices(f);
  const list = $('#plan');
  list.replaceChildren();
  f.plan.forEach((r, i) => {
    const condSel = el('select', {}, CONDS.map(([k, n]) => el('option', { value: k, selected: r.cond.k === k }, n)));
    const def = CONDS.find((c) => c[0] === r.cond.k);
    const thrSel = def && def[2]
      ? el('select', {}, THRESHOLDS.map((t) => el('option', { value: t, selected: r.cond.v === t }, `${t} %`)))
      : null;
    const actSel = el('select', {}, choices.map(([k, n]) => el('option', { value: k, selected: actionKey(r.action) === k }, n)));
    condSel.onchange = () => {
      const d = CONDS.find((c) => c[0] === condSel.value);
      r.cond = d[2] ? { k: d[0], v: 50 } : { k: d[0] };
      planChanged();
    };
    if (thrSel) thrSel.onchange = () => { r.cond.v = +thrSel.value; planChanged(); };
    actSel.onchange = () => { r.action = keyToAction(actSel.value); planChanged(); };
    const move = (d) => () => {
      const j = i + d;
      if (j < 0 || j >= f.plan.length) return;
      [f.plan[i], f.plan[j]] = [f.plan[j], f.plan[i]];
      planChanged();
    };
    list.append(el('li', { class: i >= slots ? 'off' : '' }, condSel, thrSel, el('span', { class: 'arrow' }, '→'), actSel,
      el('span', { class: 'tools' },
        el('button', { class: 'ghost small', title: 'Monter', onclick: move(-1), disabled: i === 0 }, '▲'),
        el('button', { class: 'ghost small', title: 'Descendre', onclick: move(1), disabled: i === f.plan.length - 1 }, '▼'),
        el('button', { class: 'ghost small', title: 'Supprimer', onclick: () => { f.plan.splice(i, 1); planChanged(); } }, '✕'))));
  });
  $('#add-rule').disabled = f.plan.length >= slots;
  $('#add-rule').onclick = () => { f.plan.push({ cond: { k: 'always' }, action: { k: 'attack' } }); planChanged(); };
  $('#slots').textContent = `${Math.min(f.plan.length, slots)} / ${slots} règles utilisées${f.plan.length > slots ? ' (les règles grisées sont ignorées)' : ''}.`;
}

function renderOpponents() {
  const box = $('#opponents');
  box.replaceChildren();
  const left = view.fights_left;
  $('#fights-left').textContent = left == null ? '' : `Combats restants aujourd'hui : ${left}`;
  view.opponents.forEach(({ fighter: o, bot }, i) => {
    const pred = preds[i];
    const predEl = el('span', { class: 'pred' + (pred == null ? '' : pred >= 0.6 ? ' good' : pred <= 0.4 ? ' bad' : ' mid') },
      pred == null ? '' : `${Math.round(pred * 100)} % de victoires`);
    box.append(el('div', { class: 'opp' },
      el('div', { class: 'row between' },
        el('h3', {}, o.name, backend.online ? el('span', { class: 'tag', style: 'margin-left:6px' }, bot ? 'bot' : 'joueur') : null),
        el('span', { class: 'muted' }, `Niv. ${o.level} · ${maxHp(o)} PV`)),
      el('div', { class: 'mini' }, ...Object.keys(STATS).map((k) => el('span', { title: STATS[k][0] }, `${STATS[k][0].slice(0, 3)} `, el('b', {}, o[k])))),
      el('div', { class: 'muted', style: 'font-size:12px' }, `Posture ${STANCES[o.stance][0].toLowerCase()}`),
      chipList(o),
      el('div', { class: 'actions' },
        el('button', { onclick: () => startFight(i), disabled: left === 0 }, 'Combattre'),
        el('button', { class: 'ghost small', onclick: () => {
          preds[i] = api('predict', { a: view.fighter, b: o, n: 40, seed: seed() });
          renderOpponents();
        } }, 'Pronostic'),
        predEl)));
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

// ---------- Level up ----------
function openLevelUp() {
  $('#choices').replaceChildren(...view.options.map((u, i) => {
    const [title, desc] = upgradeText(u);
    return el('button', { class: 'choice', onclick: async () => {
      $('#levelup').hidden = true;
      setView(await guarded(() => backend.level(i)));
    } }, el('b', {}, title), el('span', {}, desc));
  }));
  $('#levelup').hidden = false;
}

// ---------- Fight replay ----------
let anim = null;

async function startFight(i) {
  clearTimeout(planTimer);
  const f = view.fighter;
  await guarded(() => backend.savePlan(f.plan, f.stance));
  const r = await guarded(() => backend.fight(i));
  if (!r) return;
  const me = view.fighter;
  view = r.me;
  playFight(me, r.foe, r.result);
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

function playFight(meF, foeF, res) {
  const names = [meF.name, foeF.name];
  const plans = [meF.plan, foeF.plan];
  const canvas = $('#stage');
  const ctx = canvas.getContext('2d');
  const sides = [meF, foeF].map((f, i) => ({
    name: f.name, max: res.max_hp[i], hp: res.max_hp[i], shown: res.max_hp[i], dodge: 0, crit: 0,
    wield: null, guard: false, stunned: false, stance: f.stance,
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
      el('h3', { style: 'margin-top:8px' }, `Plan de ${foeF.name} (révélé)`),
      el('ol', {}, foeF.plan.map((rule) => el('li', {}, ruleText(rule)))));
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
    preds = {};
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

// ---------- Boot ----------
async function boot() {
  await loadEngine();
  backend = (await Remote.available()) ? Remote : Local;
  $('#mode').textContent = backend.online ? 'en ligne' : 'hors ligne (solo)';
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
    view = null;
    $('#name').value = '';
    show('create');
  };
  $('#reroll').onclick = async () => setView(await guarded(() => backend.reroll()));

  const v = await backend.me();
  if (v) {
    show('home');
    setView(v);
  } else {
    show('create');
  }
}

boot().catch((e) => {
  document.body.append(el('pre', { style: 'color:#e0574a;padding:20px' }, `Erreur de chargement : ${e.message}\nLance le jeu via un serveur local (voir README).`));
});
