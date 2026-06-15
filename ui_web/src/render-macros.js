// --- Macros dashboard rendering, macro mapping popover, and drag-drop routing ---

import { state } from './state.js';
import { modulesData, paramSpecs } from './data.js';
import { sendIPCMessage } from './ipc.js';
import { bindKnobDragging, updateKnobVisuals, updateModulatedKnobVisuals } from './utils.js';

// Populate the container so utils.js's bindKnobDragging can call the real implementation
updateModulatedKnobVisuals.fn = macroUpdateFn;
import { renderRack } from './render-rack.js';
import { renderInspector } from './render-inspector.js';

const MACRO_COLORS = ['#00D2FF', '#E07A5F', '#4ADE80', '#A27DDF'];

// --- Update all knobs that are modulated by a given macro ---

function macroUpdateFn(macroIdx, val) {
  state.macro_mappings.filter(m => m.macro_index === macroIdx).forEach(mapping => {
    const paramId = mapping.target_param_id;
    const norm = mapping.inverted ? 1.0 - val : val;
    const modulatedVal = mapping.min_value + norm * (mapping.max_value - mapping.min_value);
    let selector = `.knob-widget[data-param="${paramId}"]`;
    if (mapping.slot_id) selector += `[data-slot-id="${mapping.slot_id}"]`;
    document.querySelectorAll(selector).forEach(knob => {
      updateKnobVisuals(knob, paramId, modulatedVal);
    });
  });
}

// Re-export for callers that import from render-macros.js
export { macroUpdateFn as updateModulatedKnobVisuals };

// --- Main macros strip renderer ---

export function renderMacrosStrip() {
  const container = document.getElementById('macros-dashboard-container');
  if (!container) return;

  let html = '';
  for (let idx = 0; idx < 4; idx++) {
    const val = state.macros[idx] ?? 0.0;
    const color = MACRO_COLORS[idx];
    const mappingsCount = state.macro_mappings.filter(m => m.macro_index === idx).length;

    html += `
      <div class="macro-dashboard-item" data-index="${idx}" style="--macro-color: ${color};">
        <span class="macro-dashboard-drag-handle" data-index="${idx}" draggable="true" title="Drag to target knob to route">&#8984;</span>
        <span class="macro-dashboard-label" style="color: ${color};">M${idx + 1}</span>
        <div class="knob-widget inline-knob macro-knob" data-param="macro_${idx + 1}" data-macro-index="${idx}">
          <div class="knob-value-tooltip">${Math.round(val * 100)}%</div>
          <div class="knob-svg-container">
            <svg class="knob-svg" viewBox="0 0 32 32">
              <circle class="knob-track" cx="16" cy="16" r="13"></circle>
              <circle class="knob-value-arc" cx="16" cy="16" r="13"
                      style="stroke: ${color}; stroke-dashoffset: ${81.6 - (val * 81.6 * 0.75)}; transform: rotate(225deg); transform-origin: 50% 50%;"></circle>
            </svg>
            <div class="knob-dial-face">
              <div class="knob-pointer" style="transform: rotate(${-135 + val * 270}deg);"></div>
              <div class="knob-center-cap"></div>
            </div>
          </div>
        </div>
        <div class="macro-mappings-popover" id="macro-mappings-popover-${idx}">
          <div class="macro-mapping-popover-header">Macro ${idx + 1} Targets (${mappingsCount})</div>
          <div style="display:flex;flex-direction:column;gap:6px;max-height:220px;overflow-y:auto;" id="macro-mappings-list-${idx}"></div>
        </div>
      </div>`;
  }
  container.innerHTML = html;

  bindKnobDragging(container);

  // Right-click → popover
  container.querySelectorAll('.macro-dashboard-item').forEach(item => {
    item.addEventListener('contextmenu', e => {
      e.preventDefault();
      e.stopPropagation();
      const idx = parseInt(item.getAttribute('data-index'));
      document.querySelectorAll('.macro-mappings-popover').forEach(p => {
        if (p.id !== `macro-mappings-popover-${idx}`) p.classList.remove('show');
      });
      const popover = document.getElementById(`macro-mappings-popover-${idx}`);
      if (popover) {
        popover.classList.toggle('show');
        if (popover.classList.contains('show')) renderMacroPopoverList(idx);
      }
    });
  });

  bindMacroDraggingEvents();
}

// --- Macro handle drag (macro → knob) ---

function bindMacroDraggingEvents() {
  document.querySelectorAll('.macro-dashboard-drag-handle').forEach(handle => {
    handle.addEventListener('dragstart', e => {
      const idx = parseInt(handle.getAttribute('data-index'));
      state.dragging_macro = idx;
      document.body.classList.add('macro-drag-active');
      document.querySelectorAll('.knob-widget').forEach(knob => {
        const pid = knob.getAttribute('data-param');
        if (pid && !pid.startsWith('macro_')) knob.classList.add('macro-drag-hover-eligible');
      });
      const ghost = document.createElement('div');
      ghost.style.cssText = 'padding:4px 8px;background:#A27DDF;color:#000;border-radius:3px;font-size:9px;font-weight:bold;position:absolute;top:-9999px';
      ghost.textContent = `Assign Macro ${idx + 1}`;
      document.body.appendChild(ghost);
      e.dataTransfer.setDragImage(ghost, 0, 0);
      setTimeout(() => ghost.remove(), 0);
    });

    handle.addEventListener('dragend', () => {
      document.body.classList.remove('macro-drag-active');
      document.querySelectorAll('.knob-widget').forEach(k => {
        k.classList.remove('macro-drag-hover-eligible', 'macro-drag-hover');
      });
      state.dragging_macro = null;
    });
  });
}

// --- Global drag events (param → macro, macro → param) ---

export function bindGlobalDragDropEvents() {
  document.addEventListener('dragstart', e => {
    const handle = e.target.closest('.knob-drag-handle');
    if (!handle) return;
    const paramId = handle.getAttribute('data-param');
    const slotId = handle.getAttribute('data-slot-id');
    state.dragging_param = paramId;
    state.dragging_param_slot_id = slotId;
    document.body.classList.add('param-drag-active');
    document.querySelectorAll('.macro-dashboard-item').forEach(item => item.classList.add('macro-drop-eligible'));
    const ghost = document.createElement('div');
    ghost.style.cssText = 'padding:4px 8px;background:#00D2FF;color:#000;border-radius:3px;font-size:9px;font-weight:bold;position:absolute;top:-9999px';
    const spec = paramSpecs[paramId];
    ghost.textContent = `Route ${spec?.label ?? paramId}`;
    document.body.appendChild(ghost);
    e.dataTransfer.setDragImage(ghost, 0, 0);
    setTimeout(() => ghost.remove(), 0);
  });

  document.addEventListener('dragend', () => {
    if (state.dragging_param !== null && state.dragging_param !== undefined) {
      document.body.classList.remove('param-drag-active');
      document.querySelectorAll('.macro-dashboard-item').forEach(item => {
        item.classList.remove('macro-drop-eligible', 'macro-drop-hover');
      });
      state.dragging_param = null;
      state.dragging_param_slot_id = null;
    }
  });

  document.addEventListener('dragover', e => {
    // Macro → knob
    if (state.dragging_macro !== null && state.dragging_macro !== undefined) {
      const knob = e.target.closest('.knob-widget');
      if (knob) {
        const pid = knob.getAttribute('data-param');
        if (!pid?.startsWith('macro_')) {
          e.preventDefault();
          knob.classList.add('macro-drag-hover');
        }
      }
    }
    // Param → macro
    if (state.dragging_param !== null && state.dragging_param !== undefined) {
      const item = e.target.closest('.macro-dashboard-item');
      if (item) { e.preventDefault(); item.classList.add('macro-drop-hover'); }
    }
  });

  document.addEventListener('dragleave', e => {
    if (state.dragging_macro !== null && state.dragging_macro !== undefined) {
      const knob = e.target.closest('.knob-widget');
      if (knob && !knob.contains(e.relatedTarget)) knob.classList.remove('macro-drag-hover');
    }
    if (state.dragging_param !== null && state.dragging_param !== undefined) {
      const item = e.target.closest('.macro-dashboard-item');
      if (item && !item.contains(e.relatedTarget)) item.classList.remove('macro-drop-hover');
    }
  });

  document.addEventListener('drop', e => {
    // Macro → knob
    if (state.dragging_macro !== null && state.dragging_macro !== undefined) {
      const knob = e.target.closest('.knob-widget');
      if (knob) {
        const pid = knob.getAttribute('data-param');
        if (!pid?.startsWith('macro_')) {
          e.preventDefault();
          assignMacroToParameter(state.dragging_macro, pid, knob.getAttribute('data-slot-id'));
          flashGreen(knob);
        }
      }
    }
    // Param → macro
    if (state.dragging_param !== null && state.dragging_param !== undefined) {
      const item = e.target.closest('.macro-dashboard-item');
      if (item) {
        e.preventDefault();
        assignMacroToParameter(parseInt(item.getAttribute('data-index')), state.dragging_param, state.dragging_param_slot_id);
        flashBlue(item);
      }
    }
  });
}

function flashGreen(el) {
  el.style.transition = 'box-shadow 0.1s ease';
  el.style.boxShadow = '0 0 15px #4ADE80';
  setTimeout(() => { el.style.boxShadow = ''; el.style.transition = ''; }, 300);
}

function flashBlue(el) {
  el.style.transition = 'box-shadow 0.1s ease';
  el.style.boxShadow = '0 0 15px #00D2FF';
  setTimeout(() => { el.style.boxShadow = ''; el.style.transition = ''; }, 300);
}

// --- Assign a macro to a parameter target ---

function assignMacroToParameter(macroIdx, paramId, slotId) {
  const exists = state.macro_mappings.some(m =>
    m.macro_index === macroIdx &&
    m.target_param_id === paramId &&
    (slotId ? m.slot_id === slotId : !m.slot_id));
  if (exists) return;

  const spec = paramSpecs[paramId];
  if (!spec) return;

  const mapping = {
    macro_index: macroIdx,
    target_param_id: paramId,
    min_value: spec.min,
    max_value: spec.max,
    inverted: false
  };
  if (slotId) mapping.slot_id = slotId;

  state.macro_mappings.push(mapping);
  sendIPCMessage('save_macro_mappings', { mappings: state.macro_mappings });

  // Targeted: add the modulated class to all matching knob widgets (rack + inspector)
  const selector = `.knob-widget[data-param="${paramId}"]${slotId ? `[data-slot-id="${slotId}"]` : ''}`;
  document.querySelectorAll(selector).forEach(knob => {
    knob.classList.add('modulated', `modulated-m${macroIdx}`);
  });

  // If the popover for this macro is open, repopulate it
  const popover = document.getElementById(`macro-mappings-popover-${macroIdx}`);
  if (popover?.classList.contains('show')) renderMacroPopoverList(macroIdx);
}

// --- Macro mapping popover content ---

function renderMacroPopoverList(macroIdx) {
  const listContainer = document.getElementById(`macro-mappings-list-${macroIdx}`);
  if (!listContainer) return;

  const mappings = state.macro_mappings.filter(m => m.macro_index === macroIdx);
  let html = '';

  if (mappings.length === 0) {
    html = `<div style="text-align:center;font-size:9px;padding:12px;color:var(--text-muted);">No targets routed.<br>Drag handle to a knob.</div>`;
  } else {
    mappings.forEach(mapping => {
      const spec = paramSpecs[mapping.target_param_id];
      let label = spec?.label ?? mapping.target_param_id;
      if (mapping.slot_id) {
        const slotObj = state.routing_order.find(s => s.id === mapping.slot_id);
        label = `${label} [${slotObj?.name || slotObj?.type || mapping.slot_id}]`;
      }
      html += `
        <div class="macro-mapping-row" data-map-index="${mapping.target_param_id}::${mapping.slot_id || ''}">
          <div class="macro-mapping-row-header">
            <span class="macro-mapping-target-name">${label}</span>
            <button class="btn-mapping-delete" data-target-id="${mapping.target_param_id}" data-slot-id="${mapping.slot_id || ''}">&times;</button>
          </div>
          <div class="macro-mapping-bounds">
            <div class="macro-mapping-input-group"><label>Min</label>
              <input type="number" class="mapping-min-input" data-target-id="${mapping.target_param_id}" data-slot-id="${mapping.slot_id || ''}" step="0.1" value="${mapping.min_value.toFixed(1)}">
            </div>
            <div class="macro-mapping-input-group"><label>Max</label>
              <input type="number" class="mapping-max-input" data-target-id="${mapping.target_param_id}" data-slot-id="${mapping.slot_id || ''}" step="0.1" value="${mapping.max_value.toFixed(1)}">
            </div>
            <button class="btn-mapping-invert ${mapping.inverted ? 'active' : ''}" data-target-id="${mapping.target_param_id}" data-slot-id="${mapping.slot_id || ''}">Inv</button>
          </div>
        </div>`;
    });
  }

  // Available targets list
  const mappedKeys = mappings.map(m => m.target_param_id + '::' + (m.slot_id || ''));
  const targets = [];
  function isMapped(pid, sid) { return mappedKeys.includes(pid + '::' + (sid || '')); }

  if (!isMapped('input_gain')) targets.push({ id: 'input_gain', label: 'Global - Input Gain', slotId: null });
  if (!isMapped('output_gain')) targets.push({ id: 'output_gain', label: 'Global - Output Gain', slotId: null });
  if (!isMapped('pitch_semi')) targets.push({ id: 'pitch_semi', label: 'Global - Pitch Transpose', slotId: null });

  state.routing_order.forEach(slot => {
    const mod = modulesData[slot.type];
    mod?.knobs?.forEach(knobId => {
      if (!isMapped(knobId, slot.id)) {
        const spec = paramSpecs[knobId];
        targets.push({ id: knobId, label: `${slot.name || slot.type} - ${spec?.label || knobId}`, slotId: slot.id });
      }
    });
  });

  let opts = targets.length === 0
    ? '<option value="">No targets available</option>'
    : `<option value="">Route target...</option>${targets.map(t => {
        const v = t.slotId ? `${t.id}::${t.slotId}` : t.id;
        return `<option value="${v}">${t.label}</option>`;
      }).join('')}`;

  html += `
    <div class="macro-mapping-add-target" style="margin-top:8px;border-top:1px solid rgba(255,255,255,0.05);padding-top:8px;display:flex;gap:6px;align-items:center;">
      <select class="slots-select macro-target-select" id="macro-target-select-${macroIdx}"
              style="flex:1;font-size:9px;padding:4px;height:22px;background:rgba(0,0,0,0.4);border:1px solid rgba(255,255,255,0.08);color:#fff;border-radius:4px;">
        ${opts}
      </select>
      <button class="btn-slots-action btn-add-macro-target" data-macro-index="${macroIdx}"
              style="padding:2px 8px;font-size:9px;font-weight:700;height:22px;border-radius:4px;line-height:18px;">+</button>
    </div>`;

  listContainer.innerHTML = html;

  // Delete button
  listContainer.querySelectorAll('.btn-mapping-delete').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const tid = btn.getAttribute('data-target-id');
      const sid = btn.getAttribute('data-slot-id') || '';
      state.macro_mappings = state.macro_mappings.filter(m =>
        !(m.macro_index === macroIdx && m.target_param_id === tid && (m.slot_id || '') === sid));
      sendIPCMessage('save_macro_mappings', { mappings: state.macro_mappings });
      renderMacrosStrip();
      renderInspector();
      renderRack();
    });
  });

  // Min/max inputs
  listContainer.querySelectorAll('.mapping-min-input').forEach(input => {
    input.addEventListener('change', () => {
      const m = findMapping(input);
      if (m) { m.min_value = parseFloat(input.value); sendIPCMessage('save_macro_mappings', { mappings: state.macro_mappings }); }
    });
  });
  listContainer.querySelectorAll('.mapping-max-input').forEach(input => {
    input.addEventListener('change', () => {
      const m = findMapping(input);
      if (m) { m.max_value = parseFloat(input.value); sendIPCMessage('save_macro_mappings', { mappings: state.macro_mappings }); }
    });
  });

  // Invert toggle
  listContainer.querySelectorAll('.btn-mapping-invert').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const m = findMapping(btn);
      if (m) { m.inverted = !m.inverted; btn.classList.toggle('active', m.inverted); sendIPCMessage('save_macro_mappings', { mappings: state.macro_mappings }); }
    });
  });

  // Add target button
  const addBtn = listContainer.querySelector('.btn-add-macro-target');
  const select = document.getElementById(`macro-target-select-${macroIdx}`);
  if (addBtn && select) {
    addBtn.addEventListener('click', e => {
      e.stopPropagation();
      const val = select.value;
      if (!val) return;
      const parts = val.split('::');
      assignMacroToParameter(macroIdx, parts[0], parts.length > 1 ? parts[1] : null);
      renderMacroPopoverList(macroIdx);
    });
  }
}

function findMapping(el) {
  const tid = el.getAttribute('data-target-id');
  const sid = el.getAttribute('data-slot-id') || '';
  return state.macro_mappings.find(m => m.target_param_id === tid && (m.slot_id || '') === sid);
}
