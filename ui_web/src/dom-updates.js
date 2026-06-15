// --- Targeted DOM updates ---
// Each function mutates specific existing DOM nodes for a value-change event.
// All functions are defensive: missing nodes = no-op. They never throw.
// Initial render is still done by renderRack/renderInspector/renderSnapshotsRow/renderMacrosStrip.

import { state } from './state.js';
import { MODULE_COLORS } from './data.js';
import { formatPanText, formatDisplayVal, updateKnobVisuals } from './utils.js';

// --- Low-level helpers ---

function setCSSVar(name, val) {
  document.documentElement.style.setProperty(name, val);
}

function getSlot(slotId) {
  return state.routing_order.find(s => s.id === slotId);
}

// --- Knob visual sync across rack + inspector ---

export function updateKnobInSlot(slotId, paramId, val) {
  const selector = `.knob-widget[data-param="${paramId}"][data-slot-id="${slotId}"]`;
  document.querySelectorAll(selector).forEach(knob => {
    updateKnobVisuals(knob, paramId, val);
  });
}

// --- Bypass: rack card dim + inspector LED ---

export function updateSlotBypass(slotId, bypassed) {
  const rackSlot = document.querySelector(`.rack-slot[data-id="${slotId}"]`);
  if (rackSlot) rackSlot.classList.toggle('bypassed', bypassed);

  const slot = getSlot(slotId);
  if (!slot) return;
  const color = state.colors[slot.type] || '#00D2FF';

  const led = document.getElementById('inspector-bypass-btn')?.querySelector('.switch-led');
  if (led) {
    led.style.background = bypassed ? '#22222e' : color;
    led.style.boxShadow = bypassed ? 'none' : `0 0 10px ${color}, 0 0 20px ${color}`;
  }
  document.getElementById('inspector-bypass-btn')?.classList.toggle('bypassed', bypassed);
  document.getElementById('inspector-bypass-btn')?.classList.toggle('active', !bypassed);
}

export function updateInspectorBypassLED(slotId, bypassed) {
  updateSlotBypass(slotId, bypassed);
}

// --- Slot name: rack .slot-desc + inspector .file-display-name ---

export function updateSlotName(slotId, name) {
  const rackDesc = document.querySelector(`.rack-slot[data-id="${slotId}"] .slot-desc`);
  if (rackDesc) {
    rackDesc.textContent = name;
    rackDesc.setAttribute('title', name);
  }
  updateFileDisplay(slotId, name);
}

export function updateFileDisplay(slotId, name) {
  if (state.selected_slot_id !== slotId) return;
  const disp = document.querySelector('.file-display-name');
  if (!disp) return;
  if (disp.id) {
    const slot = getSlot(slotId);
    const isCabCapture = slot?.type === 'cab' && slot.path;
    disp.textContent = isCabCapture ? name.replace(/\.wav$/i, '') : name;
  } else {
    disp.textContent = name;
  }
  disp.setAttribute('title', name);
}

// --- Pan: rack .slot-pan-control text ---

export function updateSlotPan(slotId, pan) {
  const ctrl = document.querySelector(`.slot-pan-control[data-slot-id="${slotId}"]`);
  if (ctrl) ctrl.textContent = formatPanText(pan);
}

// --- Lane: rack-slot .half-slot class + .lane-opt active state ---

export function updateSlotLane(slotId, lane) {
  const rackSlot = document.querySelector(`.rack-slot[data-id="${slotId}"]`);
  if (rackSlot) {
    rackSlot.classList.toggle('half-slot', lane !== 'serial');
  }
  document.querySelectorAll(`.lane-opt[data-slot-id="${slotId}"]`).forEach(btn => {
    btn.classList.toggle('active', btn.getAttribute('data-lane') === lane);
  });
}

// --- Slot color: CSS variables only ---

export function updateSlotColor(slotType, color) {
  setCSSVar(`--color-${slotType}`, color);
  const selectedSlot = getSlot(state.selected_slot_id);
  if (!selectedSlot || selectedSlot.type === slotType) {
    setCSSVar('--selected-slot-color', color);
  }
  // Keep state.colors in sync
  state.colors[slotType] = color;
}

// --- Sub-param slider: range value + fill + display ---

export function updateSubParamSlider(slotId, paramId, val) {
  const selector = `.modern-slider[data-param="${paramId}"][data-slot-id="${slotId}"]`;
  document.querySelectorAll(selector).forEach(slider => {
    slider.value = Math.round(val);
    const container = slider.closest('.modern-slider-container');
    if (!container) return;
    const { paramSpecs } = window.__slotsfxData || {};
    if (!paramSpecs) return;
    const spec = paramSpecs[paramId];
    if (!spec) return;
    const norm = (val - spec.min) / (spec.max - spec.min);
    container.style.setProperty('--fill', `${Math.round(norm * 100)}%`);
    const valueEl = container.querySelector('.sub-param-value');
    if (valueEl) valueEl.textContent = formatDisplayVal(paramId, val);
  });
}

// --- Mode/Algo selector: toggle .active on .mode-btn / .algo-btn ---

export function updateInspectorModeButtons(slotId, modeVal) {
  // mode-algo group: whichever buttons exist
  document.querySelectorAll('.selector-btn.mode-btn').forEach(btn => {
    const v = parseFloat(btn.getAttribute('data-mode-val'));
    btn.classList.toggle('active', v === modeVal);
  });
  document.querySelectorAll('.selector-btn.algo-btn').forEach(btn => {
    const v = parseFloat(btn.getAttribute('data-algo-val'));
    btn.classList.toggle('active', v === modeVal);
  });
}

// --- Normalize checkbox ---

export function updateNormalizeCheckbox(slotId, val, kind) {
  const id = kind === 'amp' ? 'chk-amp-normalize' : 'chk-cab-normalize';
  const cb = document.getElementById(id);
  if (cb) cb.checked = val;
}

// --- Ping-pong switch on delay slot ---

export function updatePingPongSwitch(slotId, val) {
  const sw = document.getElementById('chk-delay-ping-pong');
  if (!sw) return;
  const next = val > 0.5;
  sw.classList.toggle('active', next);
  sw.classList.toggle('bypassed', !next);
  const led = sw.querySelector('.switch-led');
  const slot = getSlot(slotId);
  const color = (slot && state.colors[slot.type]) || '#00D2FF';
  if (led) {
    led.style.background = next ? color : '#22222e';
    led.style.boxShadow = next ? `0 0 10px ${color}, 0 0 20px ${color}` : 'none';
  }
}

// --- Modulation classes: add/remove `modulated modulated-m${idx}` ---

export function addModulationClass(slotId, paramId, macroIdx) {
  const selector = `.knob-widget[data-param="${paramId}"][data-slot-id="${slotId}"]`;
  document.querySelectorAll(selector).forEach(knob => {
    knob.classList.add('modulated', `modulated-m${macroIdx}`);
  });
}

export function removeModulationClass(slotId, paramId) {
  const selector = `.knob-widget[data-param="${paramId}"][data-slot-id="${slotId}"]`;
  document.querySelectorAll(selector).forEach(knob => {
    knob.classList.remove('modulated', 'modulated-m0', 'modulated-m1', 'modulated-m2', 'modulated-m3');
  });
}

export function updateModulationClasses(slotId, paramId, macroIdx) {
  if (macroIdx === null || macroIdx === undefined) {
    removeModulationClass(slotId, paramId);
  } else {
    addModulationClass(slotId, paramId, macroIdx);
  }
}

// --- Macro knob visual ---

export function updateMacroKnob(idx, val) {
  const selector = `.knob-widget[data-param="macro_${idx + 1}"]`;
  document.querySelectorAll(selector).forEach(knob => {
    if (knob.classList.contains('dragging')) return;
    updateKnobVisuals(knob, `macro_${idx + 1}`, val);
  });
}

// --- Snapshot wrapper: active + modified classes ---

export function updateSnapshotActive(idx) {
  document.querySelectorAll('.snapshot-item-wrapper').forEach(w => {
    w.classList.toggle('active', parseInt(w.getAttribute('data-index')) === idx);
  });
}

export function updateSnapshotModified(idx, isModified) {
  const wrapper = document.querySelector(`.snapshot-item-wrapper[data-index="${idx}"]`);
  if (wrapper) wrapper.classList.toggle('modified', isModified);
}

// --- A/B buttons ---

export function updateAbButtons(mode) {
  document.querySelectorAll('.ab-btn').forEach(btn => {
    btn.classList.toggle('active', parseInt(btn.getAttribute('data-ab-mode')) === mode);
  });
}

// --- Cab capture progress ---

export function updateCaptureProgress(pct, text, visible) {
  const container = document.getElementById('capture-progress-container');
  const bar = document.getElementById('capture-progress-bar');
  const textEl = document.getElementById('capture-progress-text');
  if (!container || !bar || !textEl) return;
  if (visible) {
    container.style.display = 'block';
    textEl.style.display = 'block';
    if (pct !== undefined) bar.style.width = `${pct}%`;
    if (text !== undefined) textEl.textContent = text;
  } else {
    container.style.display = 'none';
    textEl.style.display = 'none';
  }
}

// --- Init: keep all CSS color vars synced with state.colors ---

export function syncAllCSSColors() {
  for (const [moduleId, colorVal] of Object.entries(state.colors)) {
    setCSSVar(`--color-${moduleId}`, colorVal);
  }
  const selectedSlot = getSlot(state.selected_slot_id);
  setCSSVar('--selected-slot-color', selectedSlot ? state.colors[selectedSlot.type] : '#7e1984');
}
