'use strict';

// ---------- Drawing ----------
function draw(ctx, sides, floats, projectiles, ev, t) {
  const W = 720;
  const H = 300;
  const ground = 240;
  ctx.clearRect(0, 0, W, H);
  // arena floor
  const g = ctx.createLinearGradient(0, 150, 0, H);
  g.addColorStop(0, '#2c241c');
  g.addColorStop(1, '#4a3826');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = '#5a452e';
  ctx.fillRect(0, ground + 18, W, H);
  ctx.strokeStyle = 'rgba(255,220,170,.06)';
  for (let x = -40; x < W; x += 60) { ctx.beginPath(); ctx.moveTo(x, ground + 18); ctx.lineTo(x + 40, H); ctx.stroke(); }

  sides.forEach((s, i) => {
    let x = s.home;
    if (ev && ev.who === i && STRIKES.has(ev.kind) && ev.kind !== 'throw') x += s.dir * Math.sin(Math.PI * t) * 150;
    if (s.flash > 0) x -= s.dir * s.flash * 10;
    drawFighter(ctx, x, ground, s);
    drawBar(ctx, i === 0 ? 20 : W - 320, 18, 300, s, i === 1);
  });

  for (const p of projectiles) {
    const k = 1 - p.life;
    const x = p.from + (p.to - p.from) * k;
    const y = 150 - Math.sin(Math.PI * k) * 60;
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(k * 12);
    if (p.weapon === 'net') { drawNet(ctx, 0, 0, 16); } else { drawWeapon(ctx, p.weapon, 1); }
    ctx.restore();
  }

  ctx.textAlign = 'center';
  for (const f of floats) {
    ctx.globalAlpha = Math.max(0, Math.min(1, f.life * 1.5));
    ctx.font = `${f.big ? 'bold 26px' : 'bold 18px'} system-ui, sans-serif`;
    ctx.fillStyle = '#000';
    ctx.fillText(f.text, f.x + 1, f.y + 1);
    ctx.fillStyle = f.color;
    ctx.fillText(f.text, f.x, f.y);
  }
  ctx.globalAlpha = 1;
}

function drawBar(ctx, x, y, w, s, right) {
  ctx.font = 'bold 15px system-ui, sans-serif';
  ctx.textAlign = right ? 'right' : 'left';
  ctx.fillStyle = '#efe4d2';
  ctx.fillText(s.name, right ? x + w : x, y + 4);
  ctx.fillStyle = '#1b1714';
  ctx.fillRect(x, y + 12, w, 12);
  const pct = Math.max(0, s.shown / s.max);
  ctx.fillStyle = pct > 0.5 ? '#7cc46b' : pct > 0.25 ? '#f2b54a' : '#e0574a';
  const fw = w * pct;
  ctx.fillRect(right ? x + w - fw : x, y + 12, fw, 12);
  ctx.font = '12px system-ui, sans-serif';
  ctx.fillStyle = '#a8977f';
  ctx.fillText(`${Math.max(0, Math.round(s.shown))} / ${s.max} PV`, right ? x + w : x, y + 39);
  // Chance gauges: dodge and crit, both out of 100.
  const gauges = [['Esquive', s.dodge, '#a9c6e6'], ['Critique', s.crit, '#ffd34d']];
  gauges.forEach(([label, v, color], k) => {
    const gy = y + 48 + k * 14;
    const gw = 110;
    const gx = right ? x + w - gw : x + 60;
    ctx.textAlign = right ? 'right' : 'left';
    ctx.fillStyle = '#a8977f';
    ctx.fillText(label, right ? gx - 8 : x, gy + 7);
    ctx.fillStyle = '#1b1714';
    ctx.fillRect(gx, gy, gw, 7);
    ctx.fillStyle = color;
    const gfw = (gw * Math.min(100, v)) / 100;
    ctx.fillRect(right ? gx + gw - gfw : gx, gy, gfw, 7);
    ctx.fillStyle = '#efe4d2';
    ctx.textAlign = right ? 'right' : 'left';
    ctx.fillText(`${v}`, right ? gx - 62 : gx + gw + 6, gy + 7);
  });
}

function drawFighter(ctx, x, ground, s) {
  const d = s.dir;
  const dead = s.hp <= 0 && s.shown < 1;
  ctx.save();
  ctx.translate(x, ground);
  if (dead) { ctx.rotate(-d * 1.3); ctx.globalAlpha = 0.7; }
  // stance ring
  ctx.fillStyle = s.stance === 'aggressive' ? 'rgba(224,87,74,.35)' : s.stance === 'defensive' ? 'rgba(110,168,224,.35)' : 'rgba(0,0,0,.3)';
  ctx.beginPath(); ctx.ellipse(0, 16, 38, 8, 0, 0, Math.PI * 2); ctx.fill();
  // legs
  ctx.strokeStyle = '#3a2c20'; ctx.lineWidth = 9; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.moveTo(-10, -20); ctx.lineTo(-14, 12); ctx.moveTo(10, -20); ctx.lineTo(14, 12); ctx.stroke();
  // body
  ctx.fillStyle = s.flash > 0.5 ? '#ffffff' : s.color;
  ctx.beginPath(); ctx.ellipse(0, -48, 26, 34, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = 'rgba(0,0,0,.18)';
  ctx.beginPath(); ctx.ellipse(-d * 8, -42, 14, 24, 0, 0, Math.PI * 2); ctx.fill();
  // head
  ctx.fillStyle = s.flash > 0.5 ? '#ffffff' : '#e9c9a0';
  ctx.beginPath(); ctx.arc(d * 4, -96, 17, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = '#1b1310';
  ctx.beginPath(); ctx.arc(d * 11, -99, 2.6, 0, Math.PI * 2); ctx.fill();
  ctx.fillRect(d * 4, -88, d * 10, 2.5);
  // arm + weapon
  ctx.save();
  ctx.translate(d * 18, -56);
  ctx.scale(d, 1);
  ctx.strokeStyle = '#e9c9a0'; ctx.lineWidth = 8;
  ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(18, 6); ctx.stroke();
  ctx.translate(20, 6);
  drawWeapon(ctx, s.wield, 1);
  ctx.restore();
  // guard
  if (s.guard) {
    ctx.strokeStyle = '#a9c6e6'; ctx.lineWidth = 5;
    ctx.beginPath(); ctx.arc(d * 6, -55, 46, d > 0 ? -0.9 : Math.PI - 0.9, d > 0 ? 0.9 : Math.PI + 0.9); ctx.stroke();
  }
  // stunned
  if (s.stunned) {
    const a = performance.now() / 300;
    ctx.fillStyle = '#f2b54a';
    for (let k = 0; k < 3; k++) {
      const ang = a + (k * Math.PI * 2) / 3;
      ctx.beginPath(); ctx.arc(Math.cos(ang) * 20, -122 + Math.sin(ang) * 5, 4, 0, Math.PI * 2); ctx.fill();
    }
  }
  ctx.restore();
}

/** Draws a weapon pointing right from the origin (the hand). */
function drawWeapon(ctx, w, scale) {
  ctx.save();
  ctx.scale(scale, scale);
  ctx.lineCap = 'round';
  switch (w) {
    case 'knife':
      ctx.fillStyle = '#5b3b22'; ctx.fillRect(-4, -3, 10, 6);
      ctx.fillStyle = '#d9dde2'; ctx.beginPath(); ctx.moveTo(6, -4); ctx.lineTo(26, 0); ctx.lineTo(6, 4); ctx.fill();
      break;
    case 'sword':
      ctx.fillStyle = '#5b3b22'; ctx.fillRect(-6, -3, 12, 6);
      ctx.fillStyle = '#8a8f96'; ctx.fillRect(5, -9, 4, 18);
      ctx.fillStyle = '#d9dde2'; ctx.beginPath(); ctx.moveTo(9, -4); ctx.lineTo(52, -2); ctx.lineTo(58, 0); ctx.lineTo(52, 2); ctx.lineTo(9, 4); ctx.fill();
      break;
    case 'club':
      ctx.strokeStyle = '#6b4526'; ctx.lineWidth = 7; ctx.beginPath(); ctx.moveTo(-4, 4); ctx.lineTo(30, -14); ctx.stroke();
      ctx.fillStyle = '#7d5230'; ctx.beginPath(); ctx.ellipse(36, -18, 14, 10, -0.5, 0, Math.PI * 2); ctx.fill();
      break;
    case 'spear':
      ctx.strokeStyle = '#8a6a44'; ctx.lineWidth = 4; ctx.beginPath(); ctx.moveTo(-30, 0); ctx.lineTo(62, 0); ctx.stroke();
      ctx.fillStyle = '#d9dde2'; ctx.beginPath(); ctx.moveTo(60, -6); ctx.lineTo(80, 0); ctx.lineTo(60, 6); ctx.fill();
      break;
    case 'flail':
      ctx.strokeStyle = '#6b4526'; ctx.lineWidth = 5; ctx.beginPath(); ctx.moveTo(-4, 0); ctx.lineTo(22, -4); ctx.stroke();
      ctx.strokeStyle = '#a0a4aa'; ctx.lineWidth = 2; ctx.setLineDash([3, 2]); ctx.beginPath(); ctx.moveTo(22, -4); ctx.lineTo(38, 8); ctx.stroke(); ctx.setLineDash([]);
      ctx.fillStyle = '#6f747b'; ctx.beginPath(); ctx.arc(42, 11, 8, 0, Math.PI * 2); ctx.fill();
      ctx.strokeStyle = '#6f747b'; ctx.lineWidth = 2;
      for (let k = 0; k < 6; k++) { const a = (k * Math.PI) / 3; ctx.beginPath(); ctx.moveTo(42 + Math.cos(a) * 8, 11 + Math.sin(a) * 8); ctx.lineTo(42 + Math.cos(a) * 12, 11 + Math.sin(a) * 12); ctx.stroke(); }
      break;
    default: // fists
      ctx.fillStyle = '#e9c9a0'; ctx.beginPath(); ctx.arc(2, 0, 7, 0, Math.PI * 2); ctx.fill();
  }
  ctx.restore();
}

function drawNet(ctx, x, y, r) {
  ctx.strokeStyle = '#d8c79a'; ctx.lineWidth = 1.5;
  for (let k = -r; k <= r; k += 6) {
    ctx.beginPath(); ctx.moveTo(x + k, y - r); ctx.lineTo(x + k, y + r); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(x - r, y + k); ctx.lineTo(x + r, y + k); ctx.stroke();
  }
}

