// --- Pure utility functions ---

import { paramSpecs } from './data.js';
import { state } from './state.js';
import { sendIPCMessage } from './ipc.js';
import { syncSlotsToRust } from './ipc.js';

export { syncSlotsToRust };

// Mutable container — can't reassign a live binding, but can mutate the object
export const updateModulatedKnobVisuals = { fn: () => {} };

// --- Parameter math ---

export function getNormalizedVal(id, val) {
  const spec = paramSpecs[id];
  return (val - spec.min) / (spec.max - spec.min);
}

export function getValFromNormalized(id, norm) {
  const spec = paramSpecs[id];
  return spec.min + norm * (spec.max - spec.min);
}

export function formatDisplayVal(id, val) {
  const spec = paramSpecs[id];
  if (!spec) return val.toFixed(2);
  if (spec.type === 'db') {
    const sign = val > 0 ? '+' : '';
    return `${sign}${val.toFixed(1)} dB`;
  } else if (spec.type === 'percent') {
    return `${Math.round(val * 100)}%`;
  } else if (spec.type === 'ms') {
    return `${Math.round(val)} ms`;
  } else if (spec.type === 'semi') {
    const sign = val > 0 ? '+' : '';
    return `${sign}${Math.round(val)} st`;
  } else if (spec.type === 'hz') {
    return `${Math.round(val)} Hz`;
  }
  return val.toFixed(2);
}

export function getParamDefault(id) {
  const defaults = {
    amp_gain: 0.0, amp_bass: 0.5, amp_middle: 0.5, amp_high: 0.5, amp_output: 0.0,
    cab_gain: 0.0, cab_position: 0.5, cab_size: 0.5,
    pitch_gain: 0.0, pitch_semi: 0.0, pitch_mix: 0.5,
    delay_mix: 0.3, delay_feedback: 0.5, delay_time: 250.0, delay_ducking: 0.0, delay_ping_pong: 0.0,
    reverb_mix: 0.3, reverb_space: 0.5, reverb_shimmer: 0.5, reverb_ducking: 0.0,
    bitcrush_bits: 8.0, bitcrush_downsample: 1.0, bitcrush_mix: 0.0, bitcrush_mode: 0.0,
    overdrive_drive: 20.0, overdrive_tone: 0.5, overdrive_level: 0.5, overdrive_algo: 0.0,
    eq_low_gain: 0.0, eq_low_freq: 100.0, eq_mid_gain: 0.0, eq_mid_freq: 1000.0, eq_mid_q: 1.0, eq_high_gain: 0.0, eq_high_freq: 5000.0,
    gate_threshold: -40.0, gate_attack: 5.0, gate_release: 100.0,
    macro_1: 0.0, macro_2: 0.0, macro_3: 0.0, macro_4: 0.0,
  };
  return defaults[id] !== undefined ? defaults[id] : 0.5;
}

export function formatPanText(val) {
  if (val === 0 || val === undefined) return 'C';
  if (val < 0) return `L${Math.round(Math.abs(val) * 100)}`;
  return `R${Math.round(val * 100)}`;
}

// --- Knob visual updates ---

export function updateKnobVisuals(knob, paramId, val) {
  const norm = getNormalizedVal(paramId, val);
  const displayVal = formatDisplayVal(paramId, val);

  const tooltip = knob.querySelector('.knob-value-tooltip');
  if (tooltip) tooltip.textContent = displayVal;

  const pointer = knob.querySelector('.knob-pointer');
  if (pointer) {
    pointer.style.transform = `rotate(${-135 + norm * 270}deg)`;
  }

  const arc = knob.querySelector('.knob-value-arc');
  if (arc) {
    const isSmall = knob.classList.contains('inline-knob') || knob.classList.contains('macro-knob');
    const circumference = isSmall ? 81.6 : 125.6;
    arc.style.strokeDashoffset = circumference - (norm * circumference * 0.75);
  }
}

// --- Generic knob drag handler (used by rack, inspector, and macros) ---

export function bindKnobDragging(parentContainer) {
  parentContainer.querySelectorAll('.knob-widget').forEach(knob => {
    const paramId = knob.getAttribute('data-param');
    const isMacro = paramId && paramId.startsWith('macro_');
    const slotId = knob.getAttribute('data-slot-id');
    let slot = null;
    if (!isMacro) {
      slot = state.routing_order.find(s => s.id === slotId);
      if (!slot) return;
    }

    let startY = 0;
    let startVal = 0;

    function onMouseMove(e) {
      const deltaY = startY - e.clientY;
      const sensitivity = 0.005;
      let normVal = getNormalizedVal(paramId, startVal) + deltaY * sensitivity;
      normVal = Math.max(0, Math.min(1, normVal));
      const newVal = getValFromNormalized(paramId, normVal);

      if (isMacro) {
        const macroIdx = parseInt(paramId.split('_')[1]) - 1;
        state.macros[macroIdx] = newVal;
        updateKnobVisuals(knob, paramId, newVal);
        updateModulatedKnobVisuals.fn(macroIdx, newVal);
      } else {
        slot.params[paramId] = newVal;
        updateKnobVisuals(knob, paramId, newVal);
        document.querySelectorAll(`.knob-widget[data-param="${paramId}"][data-slot-id="${slotId}"]`).forEach(other => {
          if (other !== knob) updateKnobVisuals(other, paramId, newVal);
        });
      }

      sendIPCMessage('set_param', { param_id: paramId, value: newVal });

      if (!isMacro && slot.type === 'delay') {
        const canvas = document.getElementById('delay-echo-canvas');
        if (canvas) drawDelayEcho(canvas, slot);
      } else if (!isMacro && slot.type === 'eq') {
        const canvas = document.getElementById('eq-curve-canvas');
        if (canvas) initEqCanvas(canvas, slot);
      }
    }

    function onMouseUp() {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      knob.style.cursor = 'ns-resize';
      knob.classList.remove('dragging');
      if (!isMacro) syncSlotsToRust();
    }

    knob.addEventListener('mousedown', (e) => {
      if (e.target.closest('.knob-reassign-trigger') ||
          e.target.closest('.reassign-popover') ||
          e.target.closest('.knob-drag-handle')) return;
      startY = e.clientY;
      startVal = isMacro
        ? (state.macros[parseInt(paramId.split('_')[1]) - 1] ?? 0.0)
        : (slot.params[paramId] ?? getParamDefault(paramId));
      knob.style.cursor = 'grabbing';
      knob.classList.add('dragging');
      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    });

    knob.addEventListener('dblclick', (e) => {
      if (e.target.closest('.knob-reassign-trigger') ||
          e.target.closest('.reassign-popover') ||
          e.target.closest('.knob-drag-handle')) return;
      const defaultVal = getParamDefault(paramId);
      if (isMacro) {
        const macroIdx = parseInt(paramId.split('_')[1]) - 1;
        state.macros[macroIdx] = defaultVal;
        updateKnobVisuals(knob, paramId, defaultVal);
        updateModulatedKnobVisuals.fn(macroIdx, defaultVal);
      } else {
        slot.params[paramId] = defaultVal;
        document.querySelectorAll(`.knob-widget[data-param="${paramId}"][data-slot-id="${slotId}"]`).forEach(other => {
          updateKnobVisuals(other, paramId, defaultVal);
        });
      }
      sendIPCMessage('set_param', { param_id: paramId, value: defaultVal });
      if (!isMacro && slot.type === 'delay') {
        const canvas = document.getElementById('delay-echo-canvas');
        if (canvas) drawDelayEcho(canvas, slot);
      } else if (!isMacro && slot.type === 'eq') {
        const canvas = document.getElementById('eq-curve-canvas');
        if (canvas) initEqCanvas(canvas, slot);
      }
      syncSlotsToRust();
    });
  });
}

// --- EQ Biquad math (shared between rack visualizer and inspector) ---

export function getBiquadCoeffs(type, freq, gainDb, Q, sampleRate) {
  const w0 = 2 * Math.PI * freq / sampleRate;
  const cosW0 = Math.cos(w0);
  const sinW0 = Math.sin(w0);
  const alpha = sinW0 / (2 * Q);
  const A = Math.pow(10, gainDb / 40);
  let b0, b1, b2, a0, a1, a2;
  if (type === 'lowshelf') {
    const t = 2 * Math.sqrt(A) * alpha;
    b0 = A * ((A+1) - (A-1)*cosW0 + t);
    b1 = 2 * A * ((A-1) - (A+1)*cosW0);
    b2 = A * ((A+1) - (A-1)*cosW0 - t);
    a0 = (A+1) + (A-1)*cosW0 + t;
    a1 = -2 * ((A-1) + (A+1)*cosW0);
    a2 = (A+1) + (A-1)*cosW0 - t;
  } else if (type === 'highshelf') {
    const t = 2 * Math.sqrt(A) * alpha;
    b0 = A * ((A+1) + (A-1)*cosW0 + t);
    b1 = -2 * A * ((A-1) + (A+1)*cosW0);
    b2 = A * ((A+1) + (A-1)*cosW0 - t);
    a0 = (A+1) - (A-1)*cosW0 + t;
    a1 = 2 * ((A-1) - (A+1)*cosW0);
    a2 = (A+1) - (A-1)*cosW0 - t;
  } else { // peaking
    b0 = 1 + alpha*A; b1 = -2*cosW0; b2 = 1 - alpha*A;
    a0 = 1 + alpha/A; a1 = -2*cosW0; a2 = 1 - alpha/A;
  }
  return { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0 };
}

export function getMagnitude(c, w) {
  const cosW = Math.cos(w), sinW = Math.sin(w);
  const cos2W = Math.cos(2*w), sin2W = Math.sin(2*w);
  const numMag = Math.sqrt((c.b0+c.b1*cosW+c.b2*cos2W)**2 + (-c.b1*sinW-c.b2*sin2W)**2);
  const denMag = Math.sqrt((1+c.a1*cosW+c.a2*cos2W)**2 + (-c.a1*sinW-c.a2*sin2W)**2);
  return numMag / denMag;
}

// --- Visualizer helpers (used by rack and inspector) ---

export function drawDelayEcho(canvas, slot) {
  const ctx = canvas.getContext('2d');
  const w = canvas.width, h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  const time = slot.params.delay_time ?? 250.0;
  const feedback = slot.params.delay_feedback ?? 0.5;
  const mix = slot.params.delay_mix ?? 0.3;
  const pingPong = (slot.params.delay_ping_pong ?? 0) > 0.5;

  ctx.strokeStyle = 'rgba(255,255,255,0.06)';
  ctx.lineWidth = 1;
  for (let i = 1; i < 4; i++) {
    ctx.beginPath();
    ctx.moveTo((i/4)*w, 0);
    ctx.lineTo((i/4)*w, h);
    ctx.stroke();
  }

  ctx.lineWidth = 2;
  ctx.strokeStyle = '#fff';
  ctx.beginPath(); ctx.moveTo(15, h-10); ctx.lineTo(15, 15); ctx.stroke();

  let amp = mix, x = 15;
  const spacing = (time / 1000) * (w - 40);
  for (let i = 0; i < 6; i++) {
    x += spacing;
    if (x > w - 15) break;
    amp *= feedback;
    ctx.strokeStyle = pingPong ? (i%2===0 ? '#00ffaa' : '#ffaa00') : '#00ccff';
    ctx.shadowColor = ctx.strokeStyle;
    ctx.shadowBlur = 4;
    ctx.beginPath();
    ctx.moveTo(x, h-10);
    ctx.lineTo(x, Math.max(15, h-10-amp*(h-25)));
    ctx.stroke();
  }
  ctx.shadowBlur = 0;
}

export function initEqCanvas(canvas, slot) {
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const w = canvas.width, h = canvas.height;

  function gx(f) { const m=Math.log10; return ((m(f)-m(20))/(m(20000)-m(20)))*w; }
  function xf(x) { const m=Math.log10; return Math.pow(10, m(20)+(x/w)*(m(20000)-m(20))); }
  function gy(g) { return h - ((g+12)/24)*h; }
  function yg(y) { return -12 + ((h-y)/h)*24; }

  function draw() {
    ctx.clearRect(0, 0, w, h);
    ctx.strokeStyle = 'rgba(255,255,255,0.05)';
    [20,50,100,200,500,1000,2000,5000,10000,20000].forEach(f => {
      ctx.beginPath(); ctx.moveTo(gx(f), 0); ctx.lineTo(gx(f), h); ctx.stroke();
    });
    ctx.strokeStyle = 'rgba(255,255,255,0.15)';
    ctx.beginPath(); ctx.moveTo(0, h/2); ctx.lineTo(w, h/2); ctx.stroke();

    const cL = getBiquadCoeffs('lowshelf', slot.params.eq_low_freq??100, slot.params.eq_low_gain??0, 0.707, 44100);
    const cM = getBiquadCoeffs('peaking', slot.params.eq_mid_freq??1000, slot.params.eq_mid_gain??0, slot.params.eq_mid_q??1, 44100);
    const cH = getBiquadCoeffs('highshelf', slot.params.eq_high_freq??5000, slot.params.eq_high_gain??0, 0.707, 44100);

    ctx.strokeStyle = '#00ffaa'; ctx.lineWidth = 2.5; ctx.beginPath();
    for (let x=0; x<w; x++) {
      const ang = 2*Math.PI*xf(x)/44100;
      const m = getMagnitude(cL,ang)*getMagnitude(cM,ang)*getMagnitude(cH,ang);
      const y = gy(Math.max(-12, Math.min(12, 20*Math.log10(m))));
      x===0 ? ctx.moveTo(x,y) : ctx.lineTo(x,y);
    }
    ctx.stroke();

    const nodes = [
      { x: gx(slot.params.eq_low_freq??100), y: gy(slot.params.eq_low_gain??0), c:'#3b82f6', l:'L' },
      { x: gx(slot.params.eq_mid_freq??1000), y: gy(slot.params.eq_mid_gain??0), c:'#10b981', l:'M' },
      { x: gx(slot.params.eq_high_freq??5000), y: gy(slot.params.eq_high_gain??0), c:'#ec4899', l:'H' }
    ];
    nodes.forEach(n => {
      ctx.fillStyle = n.c;
      ctx.beginPath(); ctx.arc(n.x, n.y, 6, 0, 2*Math.PI); ctx.fill();
      ctx.strokeStyle = '#fff'; ctx.lineWidth = 1.5; ctx.stroke();
      ctx.fillStyle = '#fff'; ctx.font = 'bold 8px sans-serif';
      ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(n.l, n.x, n.y);
    });
  }

  let activeNodeIdx = -1;
  function nodeAt(mx, my) {
    return [{x:gx(slot.params.eq_low_freq??100),y:gy(slot.params.eq_low_gain??0)},
            {x:gx(slot.params.eq_mid_freq??1000),y:gy(slot.params.eq_mid_gain??0)},
            {x:gx(slot.params.eq_high_freq??5000),y:gy(slot.params.eq_high_gain??0)}]
      .findIndex(n => (mx-n.x)**2+(my-n.y)**2 <= 100);
  }

  function interact(e) {
    const rect = canvas.getBoundingClientRect();
    const mx = ((e.clientX-rect.left)/rect.width)*w;
    const my = ((e.clientY-rect.top)/rect.height)*h;
    if (activeNodeIdx===0) {
      slot.params.eq_low_freq = Math.max(20, Math.min(1000, xf(mx)));
      slot.params.eq_low_gain = Math.max(-12, Math.min(12, yg(my)));
      sendIPCMessage('set_param',{param_id:'eq_low_freq',value:slot.params.eq_low_freq});
      sendIPCMessage('set_param',{param_id:'eq_low_gain',value:slot.params.eq_low_gain});
    } else if (activeNodeIdx===1) {
      slot.params.eq_mid_freq = Math.max(200, Math.min(5000, xf(mx)));
      slot.params.eq_mid_gain = Math.max(-12, Math.min(12, yg(my)));
      sendIPCMessage('set_param',{param_id:'eq_mid_freq',value:slot.params.eq_mid_freq});
      sendIPCMessage('set_param',{param_id:'eq_mid_gain',value:slot.params.eq_mid_gain});
    } else if (activeNodeIdx===2) {
      slot.params.eq_high_freq = Math.max(1000, Math.min(20000, xf(mx)));
      slot.params.eq_high_gain = Math.max(-12, Math.min(12, yg(my)));
      sendIPCMessage('set_param',{param_id:'eq_high_freq',value:slot.params.eq_high_freq});
      sendIPCMessage('set_param',{param_id:'eq_high_gain',value:slot.params.eq_high_gain});
    }
    draw();
    document.querySelectorAll('.inspector-knobs-grid .knob-widget').forEach(k => {
      const pid = k.getAttribute('data-param');
      if (slot.params[pid] !== undefined) updateKnobVisuals(k, pid, slot.params[pid]);
    });
    document.querySelectorAll('.inspector-knobs-grid .modern-slider').forEach(s => {
      const pid = s.getAttribute('data-param');
      if (slot.params[pid] !== undefined) {
        const sp = paramSpecs[pid];
        s.value = Math.round(slot.params[pid]);
        s.closest('.modern-slider-container').style.setProperty('--fill', `${Math.round((slot.params[pid]-sp.min)/(sp.max-sp.min)*100)}%`);
        const ve = s.closest('.modern-slider-container').querySelector('.sub-param-value');
        if (ve) ve.textContent = formatDisplayVal(pid, slot.params[pid]);
      }
    });
  }

  canvas.addEventListener('mousedown', e => {
    const rect = canvas.getBoundingClientRect();
    const mx = ((e.clientX-rect.left)/rect.width)*w;
    const my = ((e.clientY-rect.top)/rect.height)*h;
    activeNodeIdx = nodeAt(mx, my);
    if (activeNodeIdx !== -1) { e.preventDefault(); interact(e); }
  });
  document.addEventListener('mousemove', e => { if (activeNodeIdx !== -1) interact(e); });
  document.addEventListener('mouseup', () => { if (activeNodeIdx !== -1) { activeNodeIdx = -1; syncSlotsToRust(); } });
  draw();
}
