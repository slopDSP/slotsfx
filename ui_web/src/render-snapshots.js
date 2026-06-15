// --- Snapshots dashboard rendering and snapshot logic ---

import { state } from './state.js';
import { sendIPCMessage } from './ipc.js';
import { syncSlotsToRust } from './ipc.js';
import { renderRack } from './render-rack.js';
import { renderInspector } from './render-inspector.js';
import { paramSpecs } from './data.js';

// --- Main snapshots row renderer ---

export function renderSnapshotsRow() {
  const container = document.getElementById('snapshots-dashboard-container');
  if (!container) return;

  let html = '';
  for (let i = 0; i < 8; i++) {
    const isActive = state.active_snapshot_index === i;
    const isModified = checkSnapshotModified(i);
    html += `
      <div class="snapshot-item-wrapper ${isActive ? 'active' : ''} ${isModified ? 'modified' : ''}" data-index="${i}">
        <button class="btn-snapshot-select" data-index="${i}">${i + 1}</button>
        <div class="snapshot-actions-dropdown" id="snapshot-actions-menu-${i}">
          <button class="snapshot-menu-item btn-save-to-snap" data-index="${i}">Store current config</button>
          <button class="snapshot-menu-item btn-clear-snap" data-index="${i}" style="color: #ff5555;">Clear snapshot</button>
        </div>
      </div>`;
  }
  container.innerHTML = html;

  // Select button → recall
  container.querySelectorAll('.btn-snapshot-select').forEach(btn => {
    btn.addEventListener('click', () => recallSnapshot(parseInt(btn.getAttribute('data-index'))));
  });

  // Right-click → show dropdown
  container.querySelectorAll('.snapshot-item-wrapper').forEach(wrapper => {
    wrapper.addEventListener('contextmenu', e => {
      e.preventDefault();
      e.stopPropagation();
      const idx = parseInt(wrapper.getAttribute('data-index'));
      document.querySelectorAll('.snapshot-actions-dropdown').forEach(m => {
        if (m.id !== `snapshot-actions-menu-${idx}`) m.classList.remove('show');
      });
      document.getElementById(`snapshot-actions-menu-${idx}`)?.classList.toggle('show');
    });
  });

  // Save to snapshot
  container.querySelectorAll('.btn-save-to-snap').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const idx = parseInt(btn.getAttribute('data-index'));
      saveCurrentConfigToSnapshot(idx);
      document.getElementById(`snapshot-actions-menu-${idx}`)?.classList.remove('show');
    });
  });

  // Clear snapshot
  container.querySelectorAll('.btn-clear-snap').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const idx = parseInt(btn.getAttribute('data-index'));
      if (confirm(`Clear Snapshot ${idx + 1}?`)) {
        clearSnapshot(idx);
      }
      document.getElementById(`snapshot-actions-menu-${idx}`)?.classList.remove('show');
    });
  });
}

// --- Snapshot dirty check ---

function checkSnapshotModified(index) {
  const snap = state.snapshots[index];
  if (!snap || !snap.slots || snap.slots.length === 0) return false;

  if (snap.slots.length !== state.routing_order.length) return true;

  for (let i = 0; i < state.routing_order.length; i++) {
    const current = state.routing_order[i];
    let saved = snap.slots[i];
    if (!saved || saved.id !== current.id) {
      saved = snap.slots.find(s => s.id === current.id);
    }
    if (!saved) return true;
    if (current.bypassed !== saved.bypassed) return true;
    if (Math.abs(current.pan - saved.pan) > 0.01) return true;
  }

  if (snap.params) {
    for (const [k, v] of Object.entries(snap.params)) {
      const liveVal = getLiveParameterValue(k);
      if (liveVal !== undefined && Math.abs(liveVal - v) > 0.01) return true;
    }
  }
  return false;
}

function getLiveParameterValue(paramId) {
  for (const slot of state.routing_order) {
    if (slot.params && slot.params[paramId] !== undefined) return slot.params[paramId];
  }
  if (paramId === 'input_gain') return state.input_gain;
  if (paramId === 'output_gain') return state.output_gain;
  if (paramId === 'pitch_semi') return state.pitch_semi;
  return undefined;
}

// --- Recall a snapshot ---

export function recallSnapshot(index) {
  state.active_snapshot_index = index;
  sendIPCMessage('set_param', { param_id: 'snapshot', value: index });

  const snap = state.snapshots[index];
  if (snap && snap.slots && snap.slots.length > 0) {
    snap.slots.forEach(savedSlot => {
      let liveSlot = state.routing_order.find(s => s.id === savedSlot.id);
      if (liveSlot) {
        liveSlot.bypassed = savedSlot.bypassed;
        liveSlot.pan = savedSlot.pan;
      }
    });

    if (snap.params) {
      Object.entries(snap.params).forEach(([paramId, val]) => {
        state.routing_order.forEach(slot => {
          if (slot.params && slot.params[paramId] !== undefined) {
            slot.params[paramId] = val;
          }
        });
        if (paramId === 'input_gain') {
          state.input_gain = val;
          updateHeaderGainUI('input', val);
        } else if (paramId === 'output_gain') {
          state.output_gain = val;
          updateHeaderGainUI('output', val);
        } else if (paramId === 'pitch_semi') {
          state.pitch_semi = val;
          updateHeaderTransposeUI(val);
        }
        sendIPCMessage('set_param', { param_id: paramId, value: val });
      });
    }

    renderRack();
    renderInspector();
    syncSlotsToRust();
  }
  renderSnapshotsRow();
}

// --- Save current config to a snapshot slot ---

export function saveCurrentConfigToSnapshot(index) {
  const slotsConfig = state.routing_order.map(s => ({
    id: s.id, bypassed: s.bypassed, pan: s.pan
  }));

  const currentParams = {};
  state.routing_order.forEach(slot => {
    if (slot.params) {
      Object.entries(slot.params).forEach(([k, v]) => { currentParams[k] = v; });
    }
  });
  currentParams['input_gain'] = state.input_gain;
  currentParams['output_gain'] = state.output_gain;
  currentParams['pitch_semi'] = state.pitch_semi;

  state.snapshots[index] = { slots: slotsConfig, params: currentParams };
  sendIPCMessage('save_snapshots', { snapshots: state.snapshots });
  renderSnapshotsRow();
}

// --- Clear a snapshot ---

export function clearSnapshot(index) {
  state.snapshots[index] = { slots: [], params: {} };
  sendIPCMessage('save_snapshots', { snapshots: state.snapshots });
  renderSnapshotsRow();
}

// --- Header UI helpers (called from recallSnapshot) ---

export function updateHeaderGainUI(type, val) {
  const pointer = document.getElementById(`header-${type}-gain-pointer`);
  const valDisplay = document.getElementById(`header-${type}-gain-value`);
  if (pointer && valDisplay) {
    pointer.style.transform = `rotate(${(val / 24) * 135}deg)`;
    valDisplay.textContent = `${val > 0 ? '+' : ''}${val.toFixed(1)} dB`;
  }
}

export function updateHeaderTransposeUI(val) {
  const pointer = document.getElementById('header-transpose-pointer');
  const valDisplay = document.getElementById('header-transpose-value');
  if (pointer && valDisplay) {
    pointer.style.transform = `rotate(${(val / 24) * 135}deg)`;
    valDisplay.textContent = val > 0 ? `+${val} st` : `${val} st`;
  }
}
