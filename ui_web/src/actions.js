// --- Action functions: user interactions that mutate state and/or send IPC ---

import { state } from './state.js';
import { sendIPCMessage } from './ipc.js';
import { syncSlotsToRust } from './ipc.js';
import { updateSlotName, updateFileDisplay } from './dom-updates.js';

// --- File browser helpers ---

export function closeFileBrowserDropdown() {
  const dropdown = document.querySelector('.file-browser-dropdown-menu');
  if (dropdown) dropdown.remove();
}

export function browseNamFile(slotId) {
  const isWebview = window.chrome && window.chrome.webview && window.chrome.webview.postMessage;
  if (isWebview) {
    sendIPCMessage('load_nam', { slot_id: slotId, filename: null });
  } else {
    const fileInput = document.getElementById('file-input-nam');
    fileInput.setAttribute('data-target-slot-id', slotId);
    fileInput.click();
  }
}

export function browseCabFile(slotId) {
  const isWebview = window.chrome && window.chrome.webview && window.chrome.webview.postMessage;
  if (isWebview) {
    sendIPCMessage('load_cab', { slot_id: slotId, filename: null });
  } else {
    const fileInput = document.getElementById('file-input-cab');
    fileInput.setAttribute('data-target-slot-id', slotId);
    fileInput.click();
  }
}

export function updateModelNameDisplay(slotId, modelName) {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (slot) {
    slot.name = modelName;
    slot.path = null;
    updateSlotName(slotId, modelName);
    updateFileDisplay(slotId, modelName);
    sendIPCMessage('load_nam', { slot_id: slotId, filename: modelName });
    syncSlotsToRust();
  }
}

export function updateIRNameDisplay(slotId, irName) {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (slot) {
    slot.name = irName;
    slot.path = null;
    updateSlotName(slotId, irName);
    updateFileDisplay(slotId, irName);
    sendIPCMessage('load_cab', { slot_id: slotId, filename: irName });
    syncSlotsToRust();
  }
}

// File input handlers (DOM events on the hidden <input> elements)
document.getElementById('file-input-nam')?.addEventListener('change', e => {
  if (e.target.files.length > 0) {
    const filename = e.target.files[0].name;
    const slotId = e.target.getAttribute('data-target-slot-id');
    updateModelNameDisplay(slotId, filename);
  }
});
document.getElementById('file-input-cab')?.addEventListener('change', e => {
  if (e.target.files.length > 0) {
    const filename = e.target.files[0].name;
    const slotId = e.target.getAttribute('data-target-slot-id');
    updateIRNameDisplay(slotId, filename);
  }
});

// --- Prev / Next model cycling ---

const DEFAULT_MODELS = ['JCM800_Crunch.nam', 'Mesa_DualRectifier.nam', 'Vox_AC30_TopBoost.nam', 'Soldano_SLO100.nam', 'Fender_TwinReverb.nam'];
const DEFAULT_IRS = ['Mesa_Recto_2x12.wav', 'Marshall_1960_4x12.wav', 'Orange_PPC412.wav', 'Fender_Deluxe_1x12.wav', 'Custom_Ribbon_Mic.wav'];

export function prevModel(slotId) {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (!slot) return;
  const cur = slot.name;
  if (DEFAULT_MODELS.includes(cur)) {
    const i = (DEFAULT_MODELS.indexOf(cur) - 1 + DEFAULT_MODELS.length) % DEFAULT_MODELS.length;
    updateModelNameDisplay(slotId, DEFAULT_MODELS[i]);
  } else {
    sendIPCMessage('prev_file', { slot_id: slotId, slot: 'nam', current_path: slot.path });
  }
}

export function nextModel(slotId) {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (!slot) return;
  const cur = slot.name;
  if (DEFAULT_MODELS.includes(cur)) {
    const i = (DEFAULT_MODELS.indexOf(cur) + 1) % DEFAULT_MODELS.length;
    updateModelNameDisplay(slotId, DEFAULT_MODELS[i]);
  } else {
    sendIPCMessage('next_file', { slot_id: slotId, slot: 'nam', current_path: slot.path });
  }
}

export function prevIR(slotId) {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (!slot) return;
  const cur = slot.name;
  if (DEFAULT_IRS.includes(cur)) {
    const i = (DEFAULT_IRS.indexOf(cur) - 1 + DEFAULT_IRS.length) % DEFAULT_IRS.length;
    updateIRNameDisplay(slotId, DEFAULT_IRS[i]);
  } else {
    sendIPCMessage('prev_file', { slot_id: slotId, slot: 'cab', current_path: slot.path });
  }
}

export function nextIR(slotId) {
  const slot = state.routing_order.find(s => s.id === slotId);
  if (!slot) return;
  const cur = slot.name;
  if (DEFAULT_IRS.includes(cur)) {
    const i = (DEFAULT_IRS.indexOf(cur) + 1) % DEFAULT_IRS.length;
    updateIRNameDisplay(slotId, DEFAULT_IRS[i]);
  } else {
    sendIPCMessage('next_file', { slot_id: slotId, slot: 'cab', current_path: slot.path });
  }
}

// --- Slot deletion: clean up snapshots and macro mappings ---

export function onSlotDeleted(deletedSlotId) {
  state.snapshots.forEach(snap => {
    if (snap && Array.isArray(snap.slots)) {
      snap.slots = snap.slots.filter(s => s.id !== deletedSlotId);
    }
  });
  state.macro_mappings = state.macro_mappings.filter(m => m.slot_id !== deletedSlotId);
  sendIPCMessage('save_snapshots', { snapshots: state.snapshots });
  sendIPCMessage('save_macro_mappings', { mappings: state.macro_mappings });
}

// --- Cabinet profiler mock sweep animation ---

export function runProfiler(slotId) {
  if (state.is_profiling) return;
  state.is_profiling = true;
  const btn = document.getElementById('btn-start-profile');
  const statusSpan = document.getElementById('profiler-status-span');
  if (btn) btn.disabled = true;
  if (statusSpan) statusSpan.textContent = 'SWEEPING LOG SINE...';

  const stages = [
    { time: 800, label: 'RECORDING CABINET RETURN...' },
    { time: 1800, label: 'DECONVOLVING RESPONSE...' },
    { time: 2600, label: 'CROPPING COEFFS (2048 SAMPLES)...' },
    { time: 3400, label: 'PROFILE CAPTURED!' }
  ];

  stages.forEach(stage => {
    setTimeout(() => {
      if (statusSpan) statusSpan.textContent = stage.label;
      if (stage.label === 'PROFILE CAPTURED!') {
        state.is_profiling = false;
        if (btn) btn.disabled = false;
        const slot = state.routing_order.find(s => s.id === slotId);
        if (slot) {
          const profiledName = 'Profiled_Cabinet_IR.wav';
          slot.name = profiledName;
          slot.path = null;
          updateSlotName(slotId, profiledName);
          sendIPCMessage('profile_captured', { slot_id: slotId, ir_name: profiledName });
          syncSlotsToRust();
        }
      }
    }, stage.time);
  });
}

// --- Modal helpers ---

export function showModal(id) {
  document.getElementById(id)?.classList.add('show');
}

export function hideModal(id) {
  document.getElementById(id)?.classList.remove('show');
}

// --- Cabinet capture events (attached when inspector renders a cab slot) ---

export function bindCabCaptureEvents(slotId) {
  const refreshBtn = document.getElementById('btn-refresh-instances');
  if (refreshBtn) {
    refreshBtn.addEventListener('click', () => sendIPCMessage('get_active_instances', {}));
  }

  function updateSweepBtn() {
    const btn = document.getElementById('btn-trigger-sweep');
    if (!btn) return;
    const hasSender = state.paired_sender_id !== undefined && state.paired_sender_id !== null;
    btn.disabled = !hasSender;
    btn.title = hasSender ? '' : 'Select a sender instance first';
  }

  const select = document.getElementById('capture-sender-select');
  if (select) {
    select.addEventListener('change', () => {
      const val = select.value;
      state.paired_sender_id = val ? parseInt(val) : null;
      sendIPCMessage('pair_instances', { sender_id: state.paired_sender_id });
      updateSweepBtn();
    });
    if (state.paired_sender_id !== undefined && state.paired_sender_id !== null) {
      select.value = state.paired_sender_id.toString();
    }
  }

  const sweepBtn = document.getElementById('btn-trigger-sweep');
  if (sweepBtn) {
    updateSweepBtn();
    sweepBtn.addEventListener('click', e => {
      e.stopPropagation();
      if (state.paired_sender_id === undefined || state.paired_sender_id === null) return;
      state.temp_capture = { active: false };
      sendIPCMessage('trigger_capture_sweep', {});
    });
  }

  // Inline rename
  const renameBtn = document.getElementById(`btn-rename-capture-${slotId}`);
  const renameIn = document.getElementById(`cab-rename-input-${slotId}`);
  const nameDisp = document.getElementById(`cab-name-display-${slotId}`);
  if (renameBtn && renameIn && nameDisp) {
    renameBtn.addEventListener('click', e => {
      e.stopPropagation();
      nameDisp.style.display = 'none';
      renameIn.style.display = 'block';
      renameIn.value = nameDisp.textContent;
      renameIn.focus();
      renameIn.select();
    });

    function commitRename() {
      const newName = renameIn.value.trim();
      if (newName && newName !== nameDisp.textContent) {
        sendIPCMessage('save_captured_ir', { name: newName });
      }
      renameIn.style.display = 'none';
      nameDisp.style.display = '';
    }
    renameIn.addEventListener('keydown', e => {
      if (e.key === 'Enter') { e.preventDefault(); commitRename(); }
      else if (e.key === 'Escape') { renameIn.style.display = 'none'; nameDisp.style.display = ''; }
    });
    renameIn.addEventListener('blur', commitRename);
  }

  // A/B test buttons
  document.querySelectorAll('.ab-btn').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const mode = parseInt(btn.getAttribute('data-ab-mode'));
      sendIPCMessage('toggle_ab_mode', { mode });
      updateAbButtonsUI(mode);
    });
  });

  updateAbButtonsUI(state.ab_mode);
}

export function updateAbButtonsUI(mode) {
  document.querySelectorAll('.ab-btn').forEach(btn => {
    const btnMode = parseInt(btn.getAttribute('data-ab-mode'));
    btn.classList.toggle('active', btnMode === mode);
  });
}
