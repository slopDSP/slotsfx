// --- SlotsFX UI — Entry Point ---
// Imports all modules and wires them together.

import './style.css';

// Core state & data
import { state } from './state.js';

// IPC bridge
import { sendIPCMessage } from './ipc.js';

// UI renderers
import { renderRack } from './render-rack.js';
import { renderInspector } from './render-inspector.js';
import { renderSnapshotsRow } from './render-snapshots.js';
import { renderMacrosStrip, bindGlobalDragDropEvents } from './render-macros.js';

// Actions (closeFileBrowserDropdown is used internally by callbacks.js)

// Init & callbacks (registers window.* functions and binds UI events)
import {
  bindHeaderTranspose,
  bindHeaderGains,
  bindVisualizersToggle,
  animateSlotVisualizers,
  bindModals,
  renderLogoDropdown,
  bindKeyboard,
  bindAddSlotButton,
} from './callbacks.js';

// --- Mount base chassis HTML ---

const appEl = document.querySelector('#app');
appEl.innerHTML = `
  <div class="chassis">
    <div class="chassis-header" style="display:flex;justify-content:space-between;align-items:center;">
      <div class="header-left" style="display:flex;align-items:center;gap:8px;">
        <span style="font-size:8px;font-weight:800;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;">IN</span>
        <div class="header-meter-vertical" id="input-meter-strip" title="Input Level">
          <div class="meter-bar-fill-vertical" id="input-meter-fill"></div>
        </div>
        <div class="header-gain-encoder" id="header-in-gain-encoder" title="Drag to adjust Input Gain">
          <div class="gain-knob"><div class="gain-pointer" id="header-in-gain-pointer"></div></div>
          <span class="gain-value" id="header-in-gain-value">0.0 dB</span>
        </div>
      </div>

      <div class="header-center" style="display:flex;align-items:center;gap:14px;">
        <div class="header-pitch-encoder" id="header-pitch-encoder" title="Drag to adjust Pitch Transpose">
          <div class="encoder-knob"><div class="encoder-pointer" id="header-transpose-pointer"></div></div>
          <span class="encoder-value" id="header-transpose-value">0 st</span>
        </div>
        <div class="logo-container-wrapper" id="logo-wrapper">
          <div class="chassis-logo" style="font-family:var(--font-header);font-size:13px;font-weight:900;letter-spacing:1.5px;color:#fff;background:linear-gradient(135deg,#FFF 0%,#A27DDF 100%);-webkit-background-clip:text;-webkit-text-fill-color:transparent;display:flex;align-items:center;gap:6px;">
            SlotsFX
            <span class="perf-status-led" id="perf-status-led" style="width:6px;height:6px;border-radius:50%;background:#00FF88;box-shadow:0 0 6px #00FF88;"></span>
          </div>
          <div class="logo-dropdown-menu" id="logo-presets-menu"></div>
        </div>
        <button class="header-spectrum-toggle" id="btn-spectrum-toggle" title="Toggle Real-Time Slot Visualizers">
          <span>VISUALS</span>
          <span class="toggle-led" id="spectrum-toggle-led"></span>
        </button>
      </div>

      <div class="header-right" style="display:flex;align-items:center;gap:8px;">
        <div class="header-gain-encoder" id="header-out-gain-encoder" title="Drag to adjust Output Gain">
          <div class="gain-knob"><div class="gain-pointer" id="header-out-gain-pointer"></div></div>
          <span class="gain-value" id="header-out-gain-value">0.0 dB</span>
        </div>
        <div class="header-meter-vertical" id="output-meter-strip" title="Output Level">
          <div class="meter-bar-fill-vertical" id="output-meter-fill"></div>
        </div>
        <span style="font-size:8px;font-weight:800;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.5px;">OUT</span>
      </div>
    </div>

    <div class="chassis-dashboard">
      <div class="dashboard-section">
        <div class="snapshots-dashboard-list" id="snapshots-dashboard-container"></div>
      </div>
      <div class="dashboard-section">
        <div class="macros-dashboard-list" id="macros-dashboard-container"></div>
      </div>
    </div>

    <div class="chassis-body">
      <div class="rack-column">
        <div class="effects-rack" id="rack-stack"></div>
        <div class="add-block-container">
          <button class="btn-add-block" id="btn-add-trigger">+ Add Slot</button>
          <div class="add-dropdown" id="add-dropdown-menu"></div>
        </div>
      </div>
      <div class="inspector-column">
        <div id="inspector-panel" style="flex:1;display:flex;flex-direction:column;overflow-y:auto;"></div>
      </div>
    </div>
  </div>

  <input type="file" id="file-input-nam" accept=".nam" style="display:none;">
  <input type="file" id="file-input-cab" accept=".wav" style="display:none;">

  <div class="slots-modal-overlay" id="modal-settings">
    <div class="slots-modal-container">
      <div class="slots-modal-header">
        <span class="slots-modal-title">SETTINGS</span>
        <button class="slots-modal-close" id="btn-close-settings">&times;</button>
      </div>
      <div class="slots-modal-body">
        <div class="slots-form-group">
          <label class="slots-form-label">NAM Models Default Directory</label>
          <div class="slots-path-row">
            <div class="slots-path-display" id="settings-nam-path">Not configured</div>
            <button class="btn-slots-clear-path" id="btn-settings-clear-nam" title="Clear">&times;</button>
            <button class="btn-slots-browse-path" id="btn-settings-browse-nam">Browse</button>
          </div>
        </div>
        <div class="slots-form-group">
          <label class="slots-form-label">Cabinet IRs Default Directory</label>
          <div class="slots-path-row">
            <div class="slots-path-display" id="settings-cab-path">Not configured</div>
            <button class="btn-slots-clear-path" id="btn-settings-clear-cab" title="Clear">&times;</button>
            <button class="btn-slots-browse-path" id="btn-settings-browse-cab">Browse</button>
          </div>
        </div>
      </div>
      <div class="slots-modal-footer">
        <button class="btn-slots-action" id="btn-save-settings">Save Settings</button>
      </div>
    </div>
  </div>

  <div class="slots-modal-overlay" id="modal-save-preset">
    <div class="slots-modal-container">
      <div class="slots-modal-header">
        <span class="slots-modal-title">SAVE PRESET</span>
        <button class="slots-modal-close" id="btn-close-save-preset">&times;</button>
      </div>
      <div class="slots-modal-body">
        <div class="slots-form-group">
          <label class="slots-form-label">Category / Folder</label>
          <input type="text" class="slots-input-text" id="preset-category-input" placeholder="e.g. Lead, Clean, Bass" list="preset-categories-datalist">
          <datalist id="preset-categories-datalist"></datalist>
        </div>
        <div class="slots-form-group">
          <label class="slots-form-label">Preset Name</label>
          <input type="text" class="slots-input-text" id="preset-name-input" placeholder="e.g. Heavy Crunch">
        </div>
      </div>
      <div class="slots-modal-footer">
        <button class="btn-slots-cancel" id="btn-cancel-save-preset">Cancel</button>
        <button class="btn-slots-action" id="btn-confirm-save-preset">Save</button>
      </div>
    </div>
  </div>

  <div class="slots-modal-overlay" id="modal-manage-presets">
    <div class="slots-modal-container">
      <div class="slots-modal-header">
        <span class="slots-modal-title">MANAGE PRESETS</span>
        <button class="slots-modal-close" id="btn-close-manage-presets">&times;</button>
      </div>
      <div class="slots-modal-body">
        <div class="preset-list-container" id="preset-manager-list"></div>
      </div>
    </div>
  </div>
`;

// Hide loading overlay
document.getElementById('slotsfx-loading')?.style.setProperty('display', 'none');

// --- CSS color sync ---

function updateCSSColors() {
  for (const [moduleId, colorVal] of Object.entries(state.colors)) {
    document.documentElement.style.setProperty(`--color-${moduleId}`, colorVal);
  }
  const selectedSlot = state.routing_order.find(s => s.id === state.selected_slot_id);
  document.documentElement.style.setProperty('--selected-slot-color',
    selectedSlot ? state.colors[selectedSlot.type] : '#7e1984');
}

// --- Initial render ---

updateCSSColors();
renderRack();
renderInspector();
renderSnapshotsRow();
renderMacrosStrip();

// --- Init bindings ---

bindHeaderTranspose();
bindHeaderGains();
bindVisualizersToggle();
requestAnimationFrame(animateSlotVisualizers);
bindModals();
renderLogoDropdown();
bindKeyboard();
bindAddSlotButton();
bindGlobalDragDropEvents();

// --- Notify Rust ---

sendIPCMessage('ui_ready', {});

// --- Metrics polling loop ---

setInterval(() => { sendIPCMessage('get_metrics', {}); }, 100);

// --- Request initial state ---

sendIPCMessage('get_settings', {});
sendIPCMessage('list_presets', {});

// --- CSS scaling ---

document.body.style.cssText = 'margin:0;padding:0;overflow:hidden;width:100vw;height:100vh;background:#0E0F15;position:relative;display:block;';

const chassis = document.querySelector('.chassis');
if (chassis) {
  chassis.style.cssText = 'width:740px;height:520px;position:absolute;transform-origin:0 0;flex-shrink:0;';
}

function applyScale() {
  if (!chassis) return;
  const baseW = 740, baseH = 520;
  const vw = window.innerWidth || 1480;
  const vh = window.innerHeight || 1040;
  const fitScale = Math.min(vw / baseW, vh / baseH);
  const finalScale = Math.min(2.0, fitScale);
  chassis.style.transform = `scale(${finalScale})`;
  const scaledW = baseW * finalScale, scaledH = baseH * finalScale;
  chassis.style.left = `${Math.max(0, (vw - scaledW) / 2)}px`;
  chassis.style.top = `${Math.max(0, (vh - scaledH) / 2)}px`;
}

window.addEventListener('resize', applyScale);
applyScale();
