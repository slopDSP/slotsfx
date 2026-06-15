// --- Rust → JS window callbacks and init bindings ---
// All window.* functions are called by Rust over the WebView IPC bridge.

import { state } from './state.js';
import { modulesData } from './data.js';
import { sendIPCMessage } from './ipc.js';
import { syncSlotsToRust } from './ipc.js';
import { updateModulatedKnobVisuals } from './utils.js';
import { renderRack } from './render-rack.js';
import { renderInspector } from './render-inspector.js';
import { renderSnapshotsRow, updateHeaderGainUI, updateHeaderTransposeUI } from './render-snapshots.js';
import { renderMacrosStrip } from './render-macros.js';
import {
  updateSlotBypass, updateSlotName, updateSlotPan, updateFileDisplay,
  updateKnobInSlot,
  updateCaptureProgress, updateAbButtons, updateMacroKnob
} from './dom-updates.js';
import { closeFileBrowserDropdown, updateModelNameDisplay, updateIRNameDisplay, onSlotDeleted, showModal, hideModal } from './actions.js';

// --- Slot sync callback (Rust → JS initial state restore) ---

window.syncSlots = (slotsJsonStr) => {
  try {
    if (JSON.stringify(state.routing_order) === slotsJsonStr) return;
    const slots = JSON.parse(slotsJsonStr);
    if (Array.isArray(slots) && slots.length > 0) {
      state.routing_order = slots;
      if (!state.routing_order.some(s => s.id === state.selected_slot_id)) {
        state.selected_slot_id = state.routing_order[0].id;
      }
      renderRack();
      renderInspector();
    }
  } catch (e) { console.error('syncSlots error:', e); }
};

// --- File loaded callback ---

window.onFileLoaded = (_type, filename, slotId, path) => {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (slot) {
    slot.name = filename;
    slot.path = path || null;
    updateSlotName(slotId, filename);
    updateFileDisplay(slotId, filename);
    syncSlotsToRust();
  }
};

// --- Directory files loaded (file browser dropdown) ---

window.onDirectoryFilesLoaded = (slotId, slotType, filesJson) => {
  try {
    const files = JSON.parse(filesJson);
    const slot = state.routing_order.find(s => s.id === slotId);
    if (!slot) return;
    closeFileBrowserDropdown();

    const container = document.querySelector('.file-browser-arrows');
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const dropdown = document.createElement('div');
    dropdown.classList.add('file-browser-dropdown-menu');
    dropdown.style.left = `${rect.left}px`;
    dropdown.style.top = `${rect.bottom + 2}px`;
    dropdown.style.width = `${rect.width}px`;

    files.forEach(file => {
      const btn = document.createElement('button');
      btn.classList.add('file-browser-dropdown-item');
      if (slot.name === file.name) btn.classList.add('active');
      btn.textContent = file.name;
      btn.title = file.name;
      btn.addEventListener('click', e => {
        e.stopPropagation();
        dropdown.remove();
        if (file.path === null) {
          if (slotType === 'nam') {
            updateModelNameDisplay(slotId, file.name);
          } else {
            updateIRNameDisplay(slotId, file.name);
          }
        } else {
          slot.name = file.name;
          slot.path = file.path;
          renderRack();
          renderInspector();
          sendIPCMessage(slotType === 'nam' ? 'load_nam' : 'load_cab', { slot_id: slotId, filename: file.path });
          syncSlotsToRust();
        }
      });
      dropdown.appendChild(btn);
    });

    document.body.appendChild(dropdown);

    const cleanup = () => {
      dropdown.remove();
      document.removeEventListener('click', cleanup);
      window.removeEventListener('resize', cleanup);
      document.getElementById('inspector-panel')?.removeEventListener('scroll', cleanup);
    };
    setTimeout(() => document.addEventListener('click', cleanup), 50);
    window.addEventListener('resize', cleanup);
    document.getElementById('inspector-panel')?.addEventListener('scroll', cleanup);
  } catch (e) { console.error('onDirectoryFilesLoaded error:', e); }
};

// --- DSP metrics update (called every 100ms) ---

window.updateDspMetrics = (procTimeNs, blockDurNs, namTimeNs, cabTimeNs, peakVal, bufSize, sRate, inputPeak, slotPeaks) => {
  const loadPercent = blockDurNs > 0 ? (procTimeNs / blockDurNs) * 100 : 0;

  const cpuLoadEl = document.getElementById('perf-cpu-load');
  if (cpuLoadEl) cpuLoadEl.textContent = `${loadPercent.toFixed(1)}%`;

  const dspTimeEl = document.getElementById('perf-block-time');
  if (dspTimeEl) dspTimeEl.textContent = `${(procTimeNs / 1000).toFixed(1)} μs`;

  const namTimeEl = document.getElementById('perf-nam-time');
  if (namTimeEl) namTimeEl.textContent = `${(namTimeNs / 1000).toFixed(1)} μs`;

  const cabTimeEl = document.getElementById('perf-cab-time');
  if (cabTimeEl) cabTimeEl.textContent = `${(cabTimeNs / 1000).toFixed(1)} μs`;

  const peakDb = peakVal > 0.00001 ? 20 * Math.log10(peakVal) : -120;
  const peakEl = document.getElementById('perf-peak');
  if (peakEl) peakEl.textContent = peakDb <= -100 ? '-inf dB' : `${peakDb.toFixed(1)} dB`;

  const srEl = document.getElementById('perf-sr');
  if (srEl) srEl.textContent = `${bufSize} smp @ ${(sRate / 1000).toFixed(1)}k`;

  const led = document.getElementById('perf-status-led');
  const bar = document.getElementById('perf-bar-fill');
  if (bar) {
    const cappedWidth = Math.min(100, loadPercent);
    bar.style.width = `${cappedWidth}%`;
    if (loadPercent > 80) {
      bar.style.backgroundColor = '#FF3333';
      bar.style.boxShadow = '0 0 6px rgba(255,51,51,0.8)';
      if (led) led.className = 'perf-status-led danger';
    } else if (loadPercent > 40) {
      bar.style.backgroundColor = '#FFAA00';
      bar.style.boxShadow = '0 0 6px rgba(255,170,0,0.8)';
      if (led) led.className = 'perf-status-led warn';
    } else {
      bar.style.backgroundColor = '#00FF88';
      bar.style.boxShadow = '0 0 6px rgba(0,255,136,0.8)';
      if (led) led.className = 'perf-status-led';
    }
  }

  // In level meter
  const inMeterFill = document.getElementById('input-meter-fill');
  if (inMeterFill && inputPeak !== undefined) {
    const db = inputPeak > 0.00001 ? 20 * Math.log10(inputPeak) : -60.0;
    const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
    inMeterFill.style.height = `${pct}%`;
    inMeterFill.style.backgroundColor = db > -3 ? '#ff3333' : db > -12 ? '#fbbf24' : '#4ade80';
  }

  // Out level meter
  const outMeterFill = document.getElementById('output-meter-fill');
  if (outMeterFill && peakVal !== undefined) {
    const db = peakVal > 0.00001 ? 20 * Math.log10(peakVal) : -60.0;
    const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
    outMeterFill.style.height = `${pct}%`;
    outMeterFill.style.backgroundColor = db > -3 ? '#ff3333' : db > -12 ? '#fbbf24' : '#4ade80';
  }

  if (slotPeaks && Array.isArray(slotPeaks)) state.current_slot_peaks = slotPeaks;
};

// --- Active instances loaded (cab capture) ---

window.onActiveInstancesLoaded = (instancesJsonStr) => {
  try {
    const list = JSON.parse(instancesJsonStr);
    state.active_instances = list;
    const select = document.getElementById('capture-sender-select');
    if (select) {
      let html = '<option value="">Pair Sender Instance...</option>';
      list.forEach(inst => {
        if (inst.id !== state.self_instance_id) html += `<option value="${inst.id}">${inst.name}</option>`;
      });
      select.innerHTML = html;
      if (state.paired_sender_id !== undefined && state.paired_sender_id !== null) {
        select.value = state.paired_sender_id.toString();
      }
    }
  } catch (e) { console.error('onActiveInstancesLoaded error:', e); }
};

// --- Captured IR saved ---

window.onCapturedIrSaved = (filename, filePath) => {
  const cabSlot = state.routing_order.find(s => s.type === 'cab');
  if (cabSlot) {
    cabSlot.name = filename;
    cabSlot.path = filePath || null;
    cabSlot.bypassed = false;
    cabSlot.params.cab_normalize = false;
    sendIPCMessage('set_bypass', { param_id: 'cab_normalize', value: false });
    if (filename !== '_temp_capture.wav') {
      state.temp_capture = null;
    } else {
      state.temp_capture = { active: true };
    }
    syncSlotsToRust();
    sendIPCMessage('load_cab', { slot_id: cabSlot.id, filename: filePath });
    updateSlotName(cabSlot.id, filename);
    updateFileDisplay(cabSlot.id, filename);
    updateSlotBypass(cabSlot.id, false);
  }
  const progressText = document.getElementById('capture-progress-text');
  if (progressText) progressText.textContent = 'IR Saved & Loaded!';
};

// --- Full state sync (Rust → JS on init) ---

window.syncAllState = (slotsJsonStr, snapshotsJsonStr, mappingsJsonStr, activeSnap, m1, m2, m3, m4, selfId) => {
  try {
    const slots = JSON.parse(slotsJsonStr);
    state.routing_order = slots;
    state.snapshots = JSON.parse(snapshotsJsonStr);
    state.macro_mappings = JSON.parse(mappingsJsonStr);
    state.active_snapshot_index = activeSnap;
    state.macros = [m1, m2, m3, m4];
    state.self_instance_id = selfId;

    const inSlot = slots.find(s => s.params?.input_gain !== undefined);
    state.input_gain = inSlot?.params?.input_gain ?? 0.0;
    const outSlot = slots.find(s => s.params?.output_gain !== undefined);
    state.output_gain = outSlot?.params?.output_gain ?? 0.0;
    const pitchSlot = slots.find(s => s.params?.pitch_semi !== undefined);
    state.pitch_semi = pitchSlot?.params?.pitch_semi ?? 0.0;

    renderSnapshotsRow();
    renderMacrosStrip();
    renderRack();
    renderInspector();

    updateHeaderGainUI('input', state.input_gain);
    updateHeaderGainUI('output', state.output_gain);
    updateHeaderTransposeUI(state.pitch_semi);
  } catch (e) { console.error('syncAllState error:', e); }
};

// --- Host state update (macros, snapshot, sweep progress, A/B mode) ---

window.updateHostState = (activeSnap, m1, m2, m3, m4, sweepProgress, abMode) => {
  if (state.active_snapshot_index !== activeSnap) {
    state.active_snapshot_index = activeSnap;
    const snap = state.snapshots[activeSnap];
    if (snap?.slots?.length > 0) {
      // Bypass + pan per slot
      snap.slots.forEach(savedSlot => {
        const live = state.routing_order.find(s => s.id === savedSlot.id);
        if (!live) return;
        live.bypassed = savedSlot.bypassed;
        live.pan = savedSlot.pan;
        updateSlotBypass(live.id, savedSlot.bypassed);
        updateSlotPan(live.id, savedSlot.pan);
      });
      // All params
      if (snap.params) {
        Object.entries(snap.params).forEach(([paramId, val]) => {
          state.routing_order.forEach(slot => {
            if (slot.params?.[paramId] !== undefined) {
              slot.params[paramId] = val;
              updateKnobInSlot(slot.id, paramId, val);
            }
          });
          if (paramId === 'input_gain') { state.input_gain = val; updateHeaderGainUI('input', val); }
          else if (paramId === 'output_gain') { state.output_gain = val; updateHeaderGainUI('output', val); }
          else if (paramId === 'pitch_semi') { state.pitch_semi = val; updateHeaderTransposeUI(val); }
        });
      }
      syncSlotsToRust();
    }
    renderSnapshotsRow();
  }

  const incomingMacros = [m1, m2, m3, m4];
  let macroChanged = false;
  for (let i = 0; i < 4; i++) {
    if (Math.abs(state.macros[i] - incomingMacros[i]) > 0.005) {
      state.macros[i] = incomingMacros[i];
      macroChanged = true;
    }
  }
  if (macroChanged) {
    for (let i = 0; i < 4; i++) {
      updateMacroKnob(i, state.macros[i]);
      updateModulatedKnobVisuals.fn(i, state.macros[i]);
    }
  }

  if (state.sweep_progress !== sweepProgress) {
    state.sweep_progress = sweepProgress;
    if (sweepProgress > 0 && sweepProgress < 999999) {
      const pct = Math.min(100, (sweepProgress / (5.2 * 48000)) * 100);
      updateCaptureProgress(pct, `Recording Sweep: ${pct.toFixed(0)}%`, true);
    } else if (sweepProgress === 999999) {
      updateCaptureProgress(100, 'FFT Deconvolution Complete!', true);
      if (state.temp_capture && !state.temp_capture.active) {
        state.temp_capture.active = true;
        sendIPCMessage('save_captured_ir', { name: '_temp_capture' });
      }
    } else {
      updateCaptureProgress(0, '', false);
    }
  }

  if (state.ab_mode !== abMode) {
    state.ab_mode = abMode;
    updateAbButtons(abMode);
  }
};

// --- Settings / presets loaded ---

let presetsDatabase = {};
let settingsState = { nam_path: null, cab_path: null };

window.onSettingsLoaded = (settingsJsonStr) => {
  try {
    settingsState = JSON.parse(settingsJsonStr);
    const namEl = document.getElementById('settings-nam-path');
    const cabEl = document.getElementById('settings-cab-path');
    if (namEl) namEl.textContent = settingsState.nam_path || 'Not configured (using default)';
    if (cabEl) cabEl.textContent = settingsState.cab_path || 'Not configured (using default)';
  } catch (e) { console.error('onSettingsLoaded error:', e); }
};

window.onPresetsLoaded = (presetsJsonStr) => {
  try {
    presetsDatabase = JSON.parse(presetsJsonStr);
    renderLogoDropdown();
    renderPresetManagerList();
  } catch (e) { console.error('onPresetsLoaded error:', e); }
};

// --- Init: Header transpose encoder ---

export function bindHeaderTranspose() {
  const encoder = document.getElementById('header-pitch-encoder');
  const pointer = document.getElementById('header-transpose-pointer');
  const valDisplay = document.getElementById('header-transpose-value');
  if (!encoder || !pointer || !valDisplay) return;

  let currentVal = 0, startY = 0, startVal = 0;

  function updateVisuals(val) {
    pointer.style.transform = `rotate(${(val / 24) * 135}deg)`;
    valDisplay.textContent = val > 0 ? `+${val} st` : `${val} st`;
  }

  encoder.addEventListener('mousedown', e => {
    startY = e.clientY; startVal = currentVal; encoder.style.cursor = 'grabbing';
    function onMouseMove(ev) {
      const stepDelta = Math.round((startY - ev.clientY) / 6);
      const newVal = Math.max(-24, Math.min(24, startVal + stepDelta));
      if (newVal !== currentVal) { currentVal = newVal; updateVisuals(currentVal); sendIPCMessage('set_header_pitch', { value: currentVal }); }
    }
    function onMouseUp() { document.removeEventListener('mousemove', onMouseMove); document.removeEventListener('mouseup', onMouseUp); encoder.style.cursor = 'ns-resize'; }
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  });

  encoder.addEventListener('dblclick', () => { currentVal = 0; updateVisuals(0); sendIPCMessage('set_header_pitch', { value: 0 }); });
  updateVisuals(0);
}

// --- Init: Header gain encoders ---

export function bindHeaderGains() {
  const inEncoder = document.getElementById('header-in-gain-encoder');
  const inPointer = document.getElementById('header-in-gain-pointer');
  const inValDisplay = document.getElementById('header-in-gain-value');
  const outEncoder = document.getElementById('header-out-gain-encoder');
  const outPointer = document.getElementById('header-out-gain-pointer');
  const outValDisplay = document.getElementById('header-out-gain-value');
  if (!inEncoder || !outEncoder) return;

  let inVal = 0.0, startYIn = 0, startValIn = 0;
  function updateInVisuals(val) {
    inPointer.style.transform = `rotate(${(val / 24) * 135}deg)`;
    inValDisplay.textContent = `${val > 0 ? '+' : ''}${val.toFixed(1)} dB`;
  }
  inEncoder.addEventListener('mousedown', e => {
    startYIn = e.clientY; startValIn = inVal; inEncoder.style.cursor = 'grabbing';
    function onMouseMove(ev) {
      const newVal = Math.max(-24.0, Math.min(24.0, startValIn + (startYIn - ev.clientY) * 0.15));
      if (newVal !== inVal) { inVal = newVal; updateInVisuals(inVal); sendIPCMessage('set_param', { param_id: 'input_gain', value: inVal }); }
    }
    function onMouseUp() { document.removeEventListener('mousemove', onMouseMove); document.removeEventListener('mouseup', onMouseUp); inEncoder.style.cursor = 'ns-resize'; syncSlotsToRust(); }
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  });
  inEncoder.addEventListener('dblclick', () => { inVal = 0.0; updateInVisuals(0.0); sendIPCMessage('set_param', { param_id: 'input_gain', value: 0.0 }); syncSlotsToRust(); });

  let outVal = 0.0, startYOut = 0, startValOut = 0;
  function updateOutVisuals(val) {
    outPointer.style.transform = `rotate(${(val / 24) * 135}deg)`;
    outValDisplay.textContent = `${val > 0 ? '+' : ''}${val.toFixed(1)} dB`;
  }
  outEncoder.addEventListener('mousedown', e => {
    startYOut = e.clientY; startValOut = outVal; outEncoder.style.cursor = 'grabbing';
    function onMouseMove(ev) {
      const newVal = Math.max(-24.0, Math.min(24.0, startValOut + (startYOut - ev.clientY) * 0.15));
      if (newVal !== outVal) { outVal = newVal; updateOutVisuals(outVal); sendIPCMessage('set_param', { param_id: 'output_gain', value: outVal }); }
    }
    function onMouseUp() { document.removeEventListener('mousemove', onMouseMove); document.removeEventListener('mouseup', onMouseUp); outEncoder.style.cursor = 'ns-resize'; syncSlotsToRust(); }
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  });
  outEncoder.addEventListener('dblclick', () => { outVal = 0.0; updateOutVisuals(0.0); sendIPCMessage('set_param', { param_id: 'output_gain', value: 0.0 }); syncSlotsToRust(); });

  updateInVisuals(0.0);
  updateOutVisuals(0.0);
}

// --- Init: Visualizers toggle ---

export function bindVisualizersToggle() {
  const btn = document.getElementById('btn-spectrum-toggle');
  if (!btn) return;
  state.visualizers_enabled = true;
  btn.classList.add('active');
  btn.addEventListener('click', () => {
    state.visualizers_enabled = !state.visualizers_enabled;
    btn.classList.toggle('active', state.visualizers_enabled);
    if (!state.visualizers_enabled) {
      document.querySelectorAll('.slot-visualizer-canvas').forEach(canvas => {
        canvas.getContext('2d').clearRect(0, 0, canvas.width, canvas.height);
      });
    }
  });
}

// --- Init: Real-time slot visualizer animation loop ---

const activeVisualizers = {};

export function animateSlotVisualizers() {
  if (!state.visualizers_enabled) { requestAnimationFrame(animateSlotVisualizers); return; }

  document.querySelectorAll('.slot-visualizer-canvas').forEach(canvas => {
    const slotId = canvas.getAttribute('data-slot-id');
    const slotIndex = state.routing_order.findIndex(s => s.id === slotId);
    if (slotIndex === -1) return;

    const peak = state.current_slot_peaks ? (state.current_slot_peaks[slotIndex] || 0.0) : 0.0;
    if (activeVisualizers[slotId] === undefined) activeVisualizers[slotId] = { currentPeak: 0.0, phase: Math.random() * 100 };
    const vis = activeVisualizers[slotId];
    vis.currentPeak += (peak - vis.currentPeak) * 0.15;
    vis.phase += 0.05 + vis.currentPeak * 0.1;

    if (canvas.width !== canvas.clientWidth || canvas.height !== canvas.clientHeight) {
      canvas.width = canvas.clientWidth; canvas.height = canvas.clientHeight;
    }
    const ctx = canvas.getContext('2d');
    const w = canvas.width, h = canvas.height;
    ctx.clearRect(0, 0, w, h);
    if (vis.currentPeak < 0.001) return;

    const slotType = state.routing_order[slotIndex].type;
    const color = state.colors[slotType] || '#00D2FF';
    ctx.strokeStyle = color; ctx.lineWidth = 1.5; ctx.beginPath();
    const amp = vis.currentPeak * h * 0.45;
    for (let x = 0; x <= w; x += 10) {
      const pct = x / w;
      const y = h / 2 + (Math.sin(pct * Math.PI * 4 + vis.phase) * 0.6 + Math.cos(pct * Math.PI * 8 - vis.phase * 0.7) * 0.4) * amp * Math.sin(pct * Math.PI);
      x === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.fillStyle = color; ctx.globalAlpha = 0.08;
    ctx.lineTo(w, h); ctx.lineTo(0, h); ctx.closePath(); ctx.fill();
    ctx.globalAlpha = 1.0;
  });

  requestAnimationFrame(animateSlotVisualizers);
}

// --- Init: Modal helpers and bindings ---

export function bindModals() {
  const logoWrapper = document.getElementById('logo-wrapper');
  const presetsMenu = document.getElementById('logo-presets-menu');
  logoWrapper?.addEventListener('click', e => {
    e.stopPropagation();
    presetsMenu?.classList.toggle('show');
    sendIPCMessage('list_presets', {});
  });

  document.addEventListener('click', e => {
    presetsMenu?.classList.remove('show');
    document.querySelectorAll('.snapshot-actions-dropdown').forEach(m => m.classList.remove('show'));
    document.querySelectorAll('.macro-mappings-popover').forEach(p => {
      if (!p.contains(e.target) && !e.target.closest('.macro-dashboard-item')) p.classList.remove('show');
    });
  });

  // Settings modal
  document.getElementById('btn-close-settings')?.addEventListener('click', () => hideModal('modal-settings'));
  document.getElementById('btn-settings-browse-nam')?.addEventListener('click', () => sendIPCMessage('browse_settings_directory', { target: 'nam' }));
  document.getElementById('btn-settings-browse-cab')?.addEventListener('click', () => sendIPCMessage('browse_settings_directory', { target: 'cab' }));
  document.getElementById('btn-settings-clear-nam')?.addEventListener('click', () => {
    settingsState.nam_path = null;
    const el = document.getElementById('settings-nam-path');
    if (el) el.textContent = 'Not configured (using default)';
    sendIPCMessage('save_settings', { settings: settingsState });
  });
  document.getElementById('btn-settings-clear-cab')?.addEventListener('click', () => {
    settingsState.cab_path = null;
    const el = document.getElementById('settings-cab-path');
    if (el) el.textContent = 'Not configured (using default)';
    sendIPCMessage('save_settings', { settings: settingsState });
  });
  document.getElementById('btn-save-settings')?.addEventListener('click', () => hideModal('modal-settings'));

  // Save preset modal
  document.getElementById('btn-close-save-preset')?.addEventListener('click', () => hideModal('modal-save-preset'));
  document.getElementById('btn-cancel-save-preset')?.addEventListener('click', () => hideModal('modal-save-preset'));
  document.getElementById('btn-confirm-save-preset')?.addEventListener('click', () => {
    const categoryInput = document.getElementById('preset-category-input');
    const nameInput = document.getElementById('preset-name-input');
    const category = categoryInput?.value.trim() || 'General';
    const name = nameInput?.value.trim();
    if (!name) { alert('Please enter a preset name.'); return; }

    const currentParams = {};
    state.routing_order.forEach(slot => {
      if (slot.params) Object.entries(slot.params).forEach(([k, v]) => { currentParams[k] = v; });
    });
    ['input_gain', 'output_gain', 'pitch_semi'].forEach(id => {
      if (!state.routing_order.some(s => s.params?.[id] !== undefined)) {
        if (id === 'input_gain') currentParams[id] = state.input_gain;
        else if (id === 'output_gain') currentParams[id] = state.output_gain;
        else if (id === 'pitch_semi') currentParams[id] = state.pitch_semi;
      }
    });

    sendIPCMessage('save_preset', { category, name, preset: { slots: state.routing_order, params: currentParams } });
    hideModal('modal-save-preset');
    if (categoryInput) categoryInput.value = '';
    if (nameInput) nameInput.value = '';
  });

  // Manage presets modal
  document.getElementById('btn-close-manage-presets')?.addEventListener('click', () => hideModal('modal-manage-presets'));
}

// --- Init: Logo dropdown presets rendering ---

export function renderLogoDropdown() {
  const dropdown = document.getElementById('logo-presets-menu');
  if (!dropdown) return;

  let html = '';
  let hasPresets = false;
  for (const [category, names] of Object.entries(presetsDatabase)) {
    if (names.length > 0) {
      hasPresets = true;
      html += `<div class="logo-dropdown-section-title">${category}</div>`;
      names.forEach(name => {
        html += `<button class="logo-dropdown-item preset-load-trigger" data-category="${category}" data-name="${name}"><span>${name}</span></button>`;
      });
    }
  }
  if (!hasPresets) html += `<div class="logo-dropdown-section-title">No local presets</div>`;

  html += `
    <button class="logo-dropdown-item logo-dropdown-action" id="dropdown-action-save">Save Preset...</button>
    <button class="logo-dropdown-item logo-dropdown-action" id="dropdown-action-manage">Manage Presets...</button>
    <button class="logo-dropdown-item logo-dropdown-action" id="dropdown-action-settings">Settings...</button>`;

  dropdown.innerHTML = html;

  dropdown.querySelectorAll('.preset-load-trigger').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      sendIPCMessage('load_preset', { category: btn.getAttribute('data-category'), name: btn.getAttribute('data-name') });
      dropdown.classList.remove('show');
    });
  });

  dropdown.querySelector('#dropdown-action-save')?.addEventListener('click', e => {
    e.stopPropagation();
    dropdown.classList.remove('show');
    showModal('modal-save-preset');
    const datalist = document.getElementById('preset-categories-datalist');
    if (datalist) datalist.innerHTML = Object.keys(presetsDatabase).map(c => `<option value="${c}">`).join('');
  });

  dropdown.querySelector('#dropdown-action-manage')?.addEventListener('click', e => {
    e.stopPropagation();
    dropdown.classList.remove('show');
    showModal('modal-manage-presets');
    renderPresetManagerList();
  });

  dropdown.querySelector('#dropdown-action-settings')?.addEventListener('click', e => {
    e.stopPropagation();
    dropdown.classList.remove('show');
    showModal('modal-settings');
  });
}

export function renderPresetManagerList() {
  const container = document.getElementById('preset-manager-list');
  if (!container) return;

  let html = '';
  let hasPresets = false;
  for (const [category, names] of Object.entries(presetsDatabase)) {
    if (names.length > 0) {
      hasPresets = true;
      html += `<div class="preset-list-category">${category}</div>`;
      names.forEach(name => {
        html += `
          <div class="preset-list-item">
            <span class="preset-item-name preset-load-trigger" data-category="${category}" data-name="${name}">${name}</span>
            <button class="btn-preset-delete" data-category="${category}" data-name="${name}">&#128465;</button>
          </div>`;
      });
    }
  }
  if (!hasPresets) html = `<div style="text-align:center;font-size:11px;padding:20px 0;color:var(--text-muted);">No presets saved yet.</div>`;

  container.innerHTML = html;

  container.querySelectorAll('.preset-load-trigger').forEach(btn => {
    btn.addEventListener('click', () => {
      sendIPCMessage('load_preset', { category: btn.getAttribute('data-category'), name: btn.getAttribute('data-name') });
      hideModal('modal-manage-presets');
    });
  });

  container.querySelectorAll('.btn-preset-delete').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      if (confirm(`Delete preset "${btn.getAttribute('data-name')}"?`)) {
        sendIPCMessage('delete_preset', { category: btn.getAttribute('data-category'), name: btn.getAttribute('data-name') });
      }
    });
  });
}

// --- Init: Keyboard shortcuts ---

export function bindKeyboard() {
  document.addEventListener('keydown', e => {
    const activeEl = document.activeElement;
    const isTextInput = activeEl && (activeEl.tagName === 'INPUT' || activeEl.tagName === 'TEXTAREA' || activeEl.isContentEditable);

    if (e.key === 'Delete' && !isTextInput && state.selected_slot_id) {
      const idx = state.routing_order.findIndex(s => s.id === state.selected_slot_id);
      if (idx !== -1) {
        onSlotDeleted(state.selected_slot_id);
        state.routing_order.splice(idx, 1);
        state.selected_slot_id = state.routing_order[idx]?.id ?? state.routing_order[idx - 1]?.id ?? '';
        renderRack();
        renderInspector();
        syncSlotsToRust();
      }
    } else if ((e.key === ' ' || e.code === 'Space') && !isTextInput) {
      e.preventDefault();
      sendIPCMessage('pass_key_to_host', { key: 'space' });
    }
  });
}

// --- Init: Add slot button + dropdown ---

export function bindAddSlotButton() {
  const btnAddTrigger = document.getElementById('btn-add-trigger');
  const addDropdownMenu = document.getElementById('add-dropdown-menu');
  if (!btnAddTrigger || !addDropdownMenu) return;

  const AVAILABLE = [
    { id: 'pitch', label: 'Pitch Shifter' }, { id: 'amp', label: 'Amp' },
    { id: 'cab', label: 'Cab' }, { id: 'delay', label: 'Delay' },
    { id: 'verb', label: 'Reverb' }, { id: 'shimmer', label: 'Cosmos' },
    { id: 'gate', label: 'Gate' }, { id: 'error', label: 'Error (Bitcrush)' },
    { id: 'od', label: 'OD (Overdrive)' }, { id: 'eq', label: 'EQ (Parametric)' }
  ];

  const DEFAULTS = {
    pitch: { pitch_gain: 0.0, pitch_semi: 0.0, pitch_mix: 0.5 },
    amp: { amp_gain: 0.0, amp_bass: 0.5, amp_middle: 0.5, amp_high: 0.5, amp_output: 0.0, amp_normalize: false },
    cab: { cab_gain: 0.0, cab_position: 0.5, cab_size: 0.5, cab_normalize: false },
    delay: { delay_time: 250.0, delay_feedback: 0.5, delay_ducking: 0.0, delay_mix: 0.3, delay_ping_pong: 0.0 },
    verb: { reverb_space: 0.5, reverb_ducking: 0.0, reverb_mix: 0.3 },
    shimmer: { reverb_mix: 0.3, reverb_space: 0.5, reverb_shimmer: 0.5 },
    gate: { gate_threshold: -40.0, gate_attack: 5.0, gate_release: 100.0 },
    error: { bitcrush_bits: 8.0, bitcrush_downsample: 1.0, bitcrush_mix: 0.5, bitcrush_mode: 0.0 },
    od: { overdrive_drive: 20.0, overdrive_tone: 0.5, overdrive_level: 0.5, overdrive_algo: 0.0 },
    eq: { eq_low_gain: 0.0, eq_low_freq: 100.0, eq_mid_gain: 0.0, eq_mid_freq: 1000.0, eq_mid_q: 1.0, eq_high_gain: 0.0, eq_high_freq: 5000.0 },
  };

  btnAddTrigger.addEventListener('click', e => {
    e.stopPropagation();
    addDropdownMenu.innerHTML = AVAILABLE.map(item =>
      `<button class="add-dropdown-item" data-type="${item.id}">+ ${item.label}</button>`).join('');
    addDropdownMenu.classList.add('show');
  });

  document.body.addEventListener('click', () => addDropdownMenu.classList.remove('show'));

  addDropdownMenu.addEventListener('click', e => {
    e.stopPropagation();
    const itemType = e.target.getAttribute('data-type');
    if (!itemType) return;
    const slotId = `${itemType}_${Date.now()}`;
    const newSlot = {
      id: slotId, type: itemType,
      name: itemType === 'amp' ? 'Empty (No Model)' : itemType === 'cab' ? 'Empty (No IR)' : modulesData[itemType]?.desc ?? itemType,
      path: null, bypassed: false, pan: 0.0, lane: 'serial',
      params: { ...(DEFAULTS[itemType] ?? {}) }
    };
    state.routing_order.push(newSlot);
    state.selected_slot_id = slotId;
    addDropdownMenu.classList.remove('show');
    renderRack();
    renderInspector();
    syncSlotsToRust();
  });
}
