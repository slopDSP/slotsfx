// --- Inspector panel (right sidebar) rendering and event bindings ---

import { state } from './state.js';
import { modulesData, paramSpecs, MODULE_COLORS } from './data.js';
import { sendIPCMessage } from './ipc.js';
import { syncSlotsToRust } from './ipc.js';
import {
  formatDisplayVal, getNormalizedVal, getValFromNormalized,
  getParamDefault, bindKnobDragging, initEqCanvas, drawDelayEcho
} from './utils.js';
import { renderRack } from './render-rack.js';
import {
  browseNamFile, browseCabFile,
  prevModel, nextModel, prevIR, nextIR,
  bindCabCaptureEvents
} from './actions.js';
import {
  updateSlotBypass, updateSlotName, updateSlotColor,
  updateInspectorModeButtons, updatePingPongSwitch
} from './dom-updates.js';

// --- Inspector renderer ---

export function renderInspector() {
  updateCSSColors();
  const inspectorEl = document.getElementById('inspector-panel');
  const slotId = state.selected_slot_id;
  const slot = state.routing_order.find(s => s.id === slotId);

  if (!slot) {
    inspectorEl.innerHTML = `
      <div class="inspector-empty">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="16" x2="12" y2="12"></line>
          <line x1="12" y1="8" x2="12.01" y2="8"></line>
        </svg>
        Select an active block to inspect its parameters.
      </div>`;
    return;
  }

  const mod = modulesData[slot.type];
  const accentColor = MODULE_COLORS[slot.type] || '#00D2FF';

  // Build knobs HTML
  let knobsHTML = '';
  mod.knobs.forEach(knobId => {
    const spec = paramSpecs[knobId];
    const val = slot.params[knobId] !== undefined ? slot.params[knobId] : getParamDefault(knobId);
    const norm = getNormalizedVal(knobId, val);
    const displayVal = formatDisplayVal(knobId, val);
    const dashOffset = 125.6 - (norm * 125.6 * 0.75);
    const angle = -135 + norm * 270;

    let subParamsHTML = '';
    if (spec?.subParams) {
      subParamsHTML = `<div class="knob-sub-params" data-parent-param="${knobId}" data-slot-id="${slot.id}">`;
      for (const [subKey, subDef] of Object.entries(spec.subParams)) {
        const subParamId = subDef.paramId;
        const subSpec = paramSpecs[subParamId];
        const subVal = slot.params[subParamId] !== undefined ? slot.params[subParamId] : subDef.default;
        const subNorm = (subVal - subSpec.min) / (subSpec.max - subSpec.min);
        subParamsHTML += `
          <div class="modern-slider-container" style="--fill: ${Math.round(subNorm*100)}%;">
            <div class="modern-slider-fill"></div>
            <input type="range" class="modern-slider"
                   data-sub-param="${subKey}" data-param="${subParamId}" data-slot-id="${slot.id}"
                   min="${subSpec.min}" max="${subSpec.max}" step="1" value="${Math.round(subVal)}">
            <div class="modern-slider-overlay">
              <span class="sub-param-value">${formatDisplayVal(subParamId, subVal)}</span>
            </div>
          </div>`;
      }
      subParamsHTML += '</div>';
    }

    const mapping = state.macro_mappings.find(m =>
      m.target_param_id === knobId && (!m.slot_id || m.slot_id === slot.id));
    const modClass = mapping ? `modulated modulated-m${mapping.macro_index}` : '';

    knobsHTML += `
      <div class="knob-with-subs" data-param="${knobId}" data-slot-id="${slot.id}">
        <div class="knob-widget ${modClass}" data-param="${knobId}" data-slot-id="${slot.id}">
          <span class="knob-drag-handle" draggable="true" data-param="${knobId}" data-slot-id="${slot.id}" title="Drag to Macro to route">&#8984;</span>
          <div class="knob-value-tooltip">${displayVal}</div>
          <div class="knob-svg-container">
            <svg class="knob-svg" viewBox="0 0 52 52">
              <circle class="knob-track" cx="26" cy="26" r="20"></circle>
              <circle class="knob-value-arc" cx="26" cy="26" r="20"
                      style="stroke-dashoffset: ${dashOffset}; transform: rotate(225deg); transform-origin: 50% 50%;"></circle>
            </svg>
            <div class="knob-dial-face">
              <div class="knob-pointer" style="transform: rotate(${angle}deg)"></div>
              <div class="knob-center-cap"></div>
            </div>
          </div>
          <div class="knob-label">${knobId.replace(/_/g, ' ')}</div>
        </div>
        ${subParamsHTML}
      </div>`;
  });

  // File panel
  let filePanelHTML = '';
  if (slot.type === 'amp') {
    const hasFile = slot.path || (slot.name !== 'Empty (No Model)' && slot.name !== 'Empty (No IR)');
    filePanelHTML = `
      <div class="inspector-file-panel">
        <div class="file-panel-label">Selected Amp Model (.nam)</div>
        <div class="file-browser-arrows">
          <button class="arrow-btn" id="btn-prev-model" data-slot-id="${slot.id}">&#9664;</button>
          <div class="file-display-name-container" style="flex:1;display:flex;align-items:center;justify-content:center;gap:6px;min-width:0;position:relative;">
            <div class="file-display-name" title="${slot.name}">${slot.name}</div>
            ${hasFile ? `<button class="file-clear-btn" id="btn-clear-slot-file" data-slot-id="${slot.id}" title="Unload file">&times;</button>` : ''}
          </div>
          <button class="arrow-btn" id="btn-next-model" data-slot-id="${slot.id}">&#9654;</button>
        </div>
        <button class="btn-file-browse" id="btn-browse-nam" data-slot-id="${slot.id}">Browse Model File</button>
      </div>`;
  } else if (slot.type === 'cab') {
    const hasFile = slot.path || (slot.name !== 'Empty (No Model)' && slot.name !== 'Empty (No IR)');
    const displayName = slot.path ? slot.name.replace(/\.wav$/i, '') : slot.name;
    filePanelHTML = `
      <div class="inspector-file-panel">
        <div class="file-panel-label">Cabinet Impulse Response</div>
        <div class="file-browser-arrows">
          <button class="arrow-btn" id="btn-prev-ir" data-slot-id="${slot.id}">&#9664;</button>
          <div class="file-display-name-container" style="flex:1;display:flex;align-items:center;justify-content:center;gap:6px;min-width:0;position:relative;">
            <div class="file-display-name" id="cab-name-display-${slot.id}" title="${slot.name}">${displayName}</div>
            ${hasFile ? `
              <button class="file-rename-btn" id="btn-rename-capture-${slot.id}" data-slot-id="${slot.id}" title="Rename capture">&#128221;</button>
              <button class="file-clear-btn" id="btn-clear-slot-file" data-slot-id="${slot.id}" title="Unload file">&times;</button>
            ` : ''}
          </div>
          <button class="arrow-btn" id="btn-next-ir" data-slot-id="${slot.id}">&#9654;</button>
        </div>
        <input type="text" id="cab-rename-input-${slot.id}" class="cab-rename-input" data-slot-id="${slot.id}"
               style="display:none;width:100%;margin-top:4px;box-sizing:border-box;" placeholder="Enter new name...">
        <button class="btn-file-browse" id="btn-browse-cab" data-slot-id="${slot.id}">Browse WAV IR</button>

        <div class="ir-capture-panel">
          <div class="panel-section-label">AUTO-IR CAPTURE (PAIRED HW/SW)</div>
          <div class="cab-capture-row">
            <select class="slots-select" id="capture-sender-select">
              <option value="">Pair Sender Instance...</option>
            </select>
            <button class="cab-btn-refresh" id="btn-refresh-instances">Refresh</button>
          </div>
          <button class="cab-btn-sweep" id="btn-trigger-sweep">Capture Sweep</button>
          <div class="cab-progress-container" id="capture-progress-container" style="display:none;">
            <div class="cab-progress-bar" id="capture-progress-bar"></div>
          </div>
          <div class="cab-progress-text" id="capture-progress-text" style="display:none;">Ready</div>
          <div id="ab-testing-section" class="ab-testing-section">
            <span>Instant A-B Test</span>
            <div class="ab-toggle-group">
              <button class="ab-btn ${state.ab_mode===0?'active':''}" data-ab-mode="0">Normal</button>
              <button class="ab-btn ${state.ab_mode===1?'active':''}" data-ab-mode="1">A (Target)</button>
              <button class="ab-btn ${state.ab_mode===2?'active':''}" data-ab-mode="2">B (IR)</button>
            </div>
          </div>
        </div>
      </div>`;
  }

  // Normalization toggles
  let normalizeHTML = '';
  if (slot.type === 'amp') {
    normalizeHTML = `
      <div class="normalize-header-toggle">
        <label class="normalize-label">
          <input type="checkbox" id="chk-amp-normalize" data-slot-id="${slot.id}"
                 ${slot.params.amp_normalize ? 'checked' : ''}>
          Normalize (-18 LUFS)
        </label>
      </div>`;
  } else if (slot.type === 'cab') {
    normalizeHTML = `
      <div class="normalize-header-toggle">
        <label class="normalize-label">
          <input type="checkbox" id="chk-cab-normalize" data-slot-id="${slot.id}"
                 ${slot.params.cab_normalize ? 'checked' : ''}>
          Normalize (0 dBFS)
        </label>
      </div>`;
  }

  // Custom panels
  let customPanelHTML = '';

  // Tail-out toggle for space effects (delay, verb, shimmer).
  const tailOut = (slot.params.tail_out ?? 0) > 0.5;
  const tailOutToggle = `
    <div style="display:flex;align-items:center;justify-content:space-between;padding-top:8px;border-top:1px solid rgba(255,255,255,0.05);margin-top:8px;">
      <span style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;">Tail Out</span>
      <label class="tail-out-toggle" title="Let the tail ring out when bypassed">
        <input type="checkbox" id="chk-tail-out" data-slot-id="${slot.id}" ${tailOut ? 'checked' : ''}>
        <span class="tail-out-track"><span class="tail-out-thumb"></span></span>
      </label>
    </div>`;
  if (slot.type === 'error') {
    const m = slot.params.bitcrush_mode ?? 0;
    customPanelHTML = `
      <div class="custom-inspector-panel" style="margin-top:15px;padding-top:15px;border-top:1px solid rgba(255,255,255,0.06);">
        <div class="panel-section-label" style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;margin-bottom:8px;">SHAPE MODE</div>
        <div class="selector-group" style="display:flex;gap:8px;">
          <button class="selector-btn mode-btn ${m<0.5?'active':''}" data-mode-val="0.0" style="flex:1;padding:6px 12px;border-radius:4px;border:1px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.3);color:#fff;font-size:11px;cursor:pointer;">Linear</button>
          <button class="selector-btn mode-btn ${m>=0.5&&m<1.5?'active':''}" data-mode-val="1.0" style="flex:1;padding:6px 12px;border-radius:4px;border:1px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.3);color:#fff;font-size:11px;cursor:pointer;">Foldback</button>
          <button class="selector-btn mode-btn ${m>=1.5?'active':''}" data-mode-val="2.0" style="flex:1;padding:6px 12px;border-radius:4px;border:1px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.3);color:#fff;font-size:11px;cursor:pointer;">Soft</button>
        </div>
      </div>`;
  } else if (slot.type === 'od') {
    const a = slot.params.overdrive_algo ?? 0;
    customPanelHTML = `
      <div class="custom-inspector-panel" style="margin-top:15px;padding-top:15px;border-top:1px solid rgba(255,255,255,0.06);">
        <div class="panel-section-label" style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;margin-bottom:8px;">DISTORTION ALGORITHM</div>
        <div class="selector-group" style="display:flex;gap:8px;">
          <button class="selector-btn algo-btn ${a<0.5?'active':''}" data-algo-val="0.0" style="flex:1;padding:6px 12px;border-radius:4px;border:1px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.3);color:#fff;font-size:11px;cursor:pointer;">TS-9</button>
          <button class="selector-btn algo-btn ${a>=0.5&&a<1.5?'active':''}" data-algo-val="1.0" style="flex:1;padding:6px 12px;border-radius:4px;border:1px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.3);color:#fff;font-size:11px;cursor:pointer;">Tanh</button>
          <button class="selector-btn algo-btn ${a>=1.5?'active':''}" data-algo-val="2.0" style="flex:1;padding:6px 12px;border-radius:4px;border:1px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.3);color:#fff;font-size:11px;cursor:pointer;">Asymmetric</button>
        </div>
      </div>`;
  } else if (slot.type === 'eq') {
    customPanelHTML = `
      <div class="custom-inspector-panel" style="margin-top:15px;padding-top:15px;border-top:1px solid rgba(255,255,255,0.06);">
        <div class="panel-section-label" style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;margin-bottom:8px;">FREQUENCY RESPONSE (DRAG BANDS)</div>
        <div class="eq-visualizer-container" style="position:relative;background:rgba(0,0,0,0.5);border-radius:6px;border:1px solid rgba(255,255,255,0.06);padding:8px;">
          <canvas id="eq-curve-canvas" width="360" height="120" style="width:100%;height:120px;display:block;cursor:crosshair;"></canvas>
        </div>
      </div>`;
  } else if (slot.type === 'delay') {
    const isPingPong = (slot.params.delay_ping_pong ?? 0) > 0.5;
    customPanelHTML = `
      <div class="custom-inspector-panel" style="margin-top:15px;padding-top:15px;border-top:1px solid rgba(255,255,255,0.06);">
        <div class="panel-section-label" style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;margin-bottom:8px;">ECHO FEEDBACK DECAY</div>
        <div class="delay-visualizer-container" style="background:rgba(0,0,0,0.5);border-radius:6px;border:1px solid rgba(255,255,255,0.06);padding:8px;margin-bottom:8px;">
          <canvas id="delay-echo-canvas" width="360" height="80" style="width:100%;height:80px;display:block;"></canvas>
        </div>
        <div style="display:flex;align-items:center;justify-content:space-between;padding-top:4px;">
          <span class="panel-section-label" style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;margin:0;">PING-PONG ROUTING</span>
          <div class="bypass-switch ${isPingPong?'active':'bypassed'}" id="chk-delay-ping-pong"
               style="width:22px;height:22px;border-radius:50%;border:1.5px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.4);display:flex;align-items:center;justify-content:center;cursor:pointer;">
            <div class="switch-led" style="width:8px;height:8px;border-radius:50%;background:${isPingPong?accentColor:'#22222e'};box-shadow:${isPingPong?`0 0 10px ${accentColor}, 0 0 20px ${accentColor}`:'none'};"></div>
          </div>
        </div>
        ${tailOutToggle}
      </div>`;
  } else if (slot.type === 'verb' || slot.type === 'shimmer') {
    customPanelHTML = `
      <div class="custom-inspector-panel" style="margin-top:15px;padding-top:15px;border-top:1px solid rgba(255,255,255,0.06);">
        <div class="panel-section-label" style="font-size:10px;font-weight:700;color:var(--text-muted);text-transform:uppercase;margin-bottom:8px;">${slot.type === 'shimmer' ? 'SHIMMER REVERB' : 'REVERB'}</div>
        ${tailOutToggle}
      </div>`;
  }

  inspectorEl.innerHTML = `
    <div class="inspector-active" data-accent="${slot.type}" style="--accent: ${accentColor};">
      <div class="inspector-header">
        <div class="inspector-header-main">
          <div class="inspector-header-left">
            <h2 class="inspector-title">${mod?.title ?? slot.type}
              <input type="color" class="color-picker-input" data-module-color="${slot.type}" value="${state.colors[slot.type]}">
            </h2>
            <span class="inspector-desc">${mod?.desc ?? ''}</span>
          </div>
          <div class="inspector-header-center">${normalizeHTML}</div>
          <div class="inspector-header-right" style="display:flex;align-items:center;justify-content:flex-end;">
            <div class="bypass-switch ${slot.bypassed?'bypassed':'active'}" id="inspector-bypass-btn" data-bypass-slot-id="${slot.id}"
                 style="width:22px;height:22px;border-radius:50%;border:1.5px solid rgba(255,255,255,0.08);background:rgba(0,0,0,0.4);display:flex;align-items:center;justify-content:center;cursor:pointer;">
              <div class="switch-led" style="width:8px;height:8px;border-radius:50%;background:${slot.bypassed?'#22222e':accentColor};box-shadow:${slot.bypassed?'none':`0 0 10px ${accentColor}, 0 0 20px ${accentColor}`};"></div>
            </div>
          </div>
        </div>
      </div>
      <div class="inspector-knobs-grid">${knobsHTML}</div>
      ${filePanelHTML}
      ${customPanelHTML}
    </div>`;

  bindInspectorEvents();

  if (slot.type === 'eq') {
    initEqCanvas(document.getElementById('eq-curve-canvas'), slot);
  } else if (slot.type === 'delay') {
    const canvas = document.getElementById('delay-echo-canvas');
    if (canvas) drawDelayEcho(canvas, slot);
  }
}

function updateCSSColors() {
  for (const [moduleId, colorVal] of Object.entries(state.colors)) {
    document.documentElement.style.setProperty(`--color-${moduleId}`, colorVal);
  }
  const selectedSlot = state.routing_order.find(s => s.id === state.selected_slot_id);
  document.documentElement.style.setProperty('--selected-slot-color',
    selectedSlot ? state.colors[selectedSlot.type] : '#7e1984');
}

// --- Inspector event bindings ---

export function bindInspectorEvents() {
  bindKnobDragging(document.getElementById('inspector-panel'));
  const inspectorEl = document.getElementById('inspector-panel');
  const slotId = state.selected_slot_id;
  const slot = state.routing_order.find(s => s.id === slotId);
  if (!slot) return;

  // Color picker
  const colorPicker = inspectorEl.querySelector(`.color-picker-input[data-module-color="${slot.type}"]`);
  if (colorPicker) {
    colorPicker.addEventListener('input', e => {
      updateSlotColor(slot.type, e.target.value);
    });
    colorPicker.addEventListener('change', () => renderRack());
  }

  // Prev/Next arrows
  document.getElementById('btn-prev-model')?.addEventListener('click', () => prevModel(slotId));
  document.getElementById('btn-next-model')?.addEventListener('click', () => nextModel(slotId));
  document.getElementById('btn-prev-ir')?.addEventListener('click', () => prevIR(slotId));
  document.getElementById('btn-next-ir')?.addEventListener('click', () => nextIR(slotId));

  // Clear file
  document.getElementById('btn-clear-slot-file')?.addEventListener('click', e => {
    e.stopPropagation();
    closeFileBrowserDropdown();
    const emptyName = slot.type === 'amp' ? 'Empty (No Model)' : 'Empty (No IR)';
    slot.name = emptyName;
    slot.path = null;
    updateSlotName(slotId, emptyName);
    syncSlotsToRust();
  });

  // File browser arrows container (triggers dropdown)
  inspectorEl.querySelector('.file-browser-arrows')?.addEventListener('click', e => {
    if (e.target.closest('.arrow-btn') || e.target.closest('.file-clear-btn')) return;
    e.stopPropagation();
    closeFileBrowserDropdown();
    sendIPCMessage('get_directory_files', { slot_id: slot.id, slot: slot.type, current_path: slot.path });
  });

  // Browse buttons
  document.getElementById('btn-browse-nam')?.addEventListener('click', () => browseNamFile(slotId));
  document.getElementById('btn-browse-cab')?.addEventListener('click', () => browseCabFile(slotId));

  // Cab capture
  if (slot.type === 'cab') bindCabCaptureEvents(slotId);

  // Bypass switch
  document.getElementById('inspector-bypass-btn')?.addEventListener('click', e => {
    e.stopPropagation();
    slot.bypassed = !slot.bypassed;
    updateSlotBypass(slotId, slot.bypassed);
    syncSlotsToRust();
  });

  // Normalization checkboxes — already reflect state in the DOM via e.target
  document.getElementById('chk-amp-normalize')?.addEventListener('change', e => {
    slot.params.amp_normalize = e.target.checked;
    sendIPCMessage('set_bypass', { param_id: 'amp_normalize', value: e.target.checked });
    syncSlotsToRust();
  });
  document.getElementById('chk-cab-normalize')?.addEventListener('change', e => {
    slot.params.cab_normalize = e.target.checked;
    sendIPCMessage('set_bypass', { param_id: 'cab_normalize', value: e.target.checked });
    syncSlotsToRust();
  });

  // Tail-out toggle for space effects
  document.getElementById('chk-tail-out')?.addEventListener('change', e => {
    const tailSlotId = e.target.getAttribute('data-slot-id');
    const tailSlot = state.routing_order.find(s => s.id === tailSlotId);
    if (tailSlot) {
      tailSlot.params.tail_out = e.target.checked ? 1.0 : 0.0;
      sendIPCMessage('set_param', { param_id: 'tail_out', value: e.target.checked });
      syncSlotsToRust();
    }
  });

  // Delay ping-pong toggle
  document.getElementById('chk-delay-ping-pong')?.addEventListener('click', () => {
    const current = (slot.params.delay_ping_pong ?? 0) > 0.5;
    const next = !current;
    slot.params.delay_ping_pong = next ? 1.0 : 0.0;
    updatePingPongSwitch(slotId, slot.params.delay_ping_pong);
    sendIPCMessage('set_bypass', { param_id: 'delay_ping_pong', value: next });
    syncSlotsToRust();
    const canvas = document.getElementById('delay-echo-canvas');
    if (canvas) drawDelayEcho(canvas, slot);
  });

  // Mode / Algo selectors
  inspectorEl.querySelectorAll('.mode-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const val = parseFloat(btn.getAttribute('data-mode-val'));
      slot.params.bitcrush_mode = val;
      sendIPCMessage('set_param', { param_id: 'bitcrush_mode', value: val });
      syncSlotsToRust();
      updateInspectorModeButtons(slotId, val);
    });
  });
  inspectorEl.querySelectorAll('.algo-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const val = parseFloat(btn.getAttribute('data-algo-val'));
      slot.params.overdrive_algo = val;
      sendIPCMessage('set_param', { param_id: 'overdrive_algo', value: val });
      syncSlotsToRust();
      updateInspectorModeButtons(slotId, val);
    });
  });

  // Sub-param sliders
  inspectorEl.querySelectorAll('.modern-slider').forEach(slider => {
    const subParamId = slider.getAttribute('data-param');
    const sliderSlot = state.routing_order.find(s => s.id === slider.getAttribute('data-slot-id'));
    if (!sliderSlot) return;
    const subSpec = paramSpecs[subParamId];

    slider.addEventListener('input', e => {
      const val = parseFloat(e.target.value);
      sliderSlot.params[subParamId] = val;
      const norm = (val - subSpec.min) / (subSpec.max - subSpec.min);
      e.target.closest('.modern-slider-container').style.setProperty('--fill', `${Math.round(norm*100)}%`);
      const valueEl = e.target.closest('.modern-slider-container').querySelector('.sub-param-value');
      if (valueEl) valueEl.textContent = formatDisplayVal(subParamId, val);
      sendIPCMessage('set_param', { param_id: subParamId, value: val });
      if (sliderSlot.type === 'eq') {
        const canvas = document.getElementById('eq-curve-canvas');
        if (canvas) initEqCanvas(canvas, sliderSlot);
      }
    });

    slider.addEventListener('change', () => syncSlotsToRust());
  });
}
