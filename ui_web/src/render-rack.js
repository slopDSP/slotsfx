// --- Rack rendering and rack-level event bindings ---

import { state } from './state.js';
import { modulesData } from './data.js';
import { syncSlotsToRust } from './ipc.js';
import {
  getNormalizedVal, formatDisplayVal, formatPanText, getParamDefault,
  bindKnobDragging
} from './utils.js';
import {
  browseNamFile, browseCabFile, prevModel, nextModel,
  prevIR, nextIR, onSlotDeleted, closeFileBrowserDropdown
} from './actions.js';
import { renderInspector } from './render-inspector.js';
import { updateSlotBypass, updateSlotPan } from './dom-updates.js';

// --- HTML generators ---

function createSlotHTML(slot) {
  const mod = modulesData[slot.type];
  const lane = slot.lane || 'serial';
  const isHalfSlot = lane !== 'serial';
  const panVal = slot.pan !== undefined ? slot.pan : 0.0;
  const panText = formatPanText(panVal);

  let inlineKnobsHTML = '';
  if (!isHalfSlot) {
    const assignedKnobs = state.slot_knobs[slot.type] || [];
    assignedKnobs.slice(0, 3).forEach((knobId, knobIdx) => {
      const spec = modulesData[slot.type]?.knobs?.includes(knobId) ? { label: knobId } : null;
      if (!spec) return;
      const val = slot.params[knobId] !== undefined ? slot.params[knobId] : getParamDefault(knobId);
      const norm = getNormalizedVal(knobId, val);
      const displayVal = formatDisplayVal(knobId, val);
      const dashOffset = 81.6 - (norm * 81.6 * 0.75);
      const angle = -135 + norm * 270;

      const mapping = state.macro_mappings.find(m =>
        m.target_param_id === knobId && (!m.slot_id || m.slot_id === slot.id));
      const modClass = mapping ? `modulated modulated-m${mapping.macro_index}` : '';

      inlineKnobsHTML += `
        <div class="knob-widget inline-knob ${modClass}" data-param="${knobId}" data-slot-id="${slot.id}" data-knob-index="${knobIdx}">
          <span class="knob-drag-handle" draggable="true" data-param="${knobId}" data-slot-id="${slot.id}" title="Drag to Macro to route">&#8984;</span>
          <div class="knob-value-tooltip">${displayVal}</div>
          <div class="knob-svg-container">
            <svg class="knob-svg" viewBox="0 0 32 32">
              <circle class="knob-track" cx="16" cy="16" r="13"></circle>
              <circle class="knob-value-arc" cx="16" cy="16" r="13"
                      style="stroke-dashoffset: ${dashOffset}; transform: rotate(225deg); transform-origin: 50% 50%;"></circle>
            </svg>
            <div class="knob-dial-face">
              <div class="knob-pointer" style="transform: rotate(${angle}deg)"></div>
              <div class="knob-center-cap"></div>
            </div>
          </div>
          <div class="knob-label knob-reassign-trigger" data-slot-id="${slot.id}" data-knob-index="${knobIdx}">${knobId.replace(/_/g, ' ')}</div>
        </div>`;
    });
  }

  const arrowsHTML = slot.type === 'amp'
    ? `<span class="slot-arrow prev-model-btn" data-slot-id="${slot.id}">&#9664;</span>
       <span class="slot-browse browse-nam-btn" data-slot-id="${slot.id}">&#128196;</span>
       <span class="slot-arrow next-model-btn" data-slot-id="${slot.id}">&#9654;</span>`
    : slot.type === 'cab'
      ? `<span class="slot-arrow prev-ir-btn" data-slot-id="${slot.id}">&#9664;</span>
       <span class="slot-browse browse-cab-btn" data-slot-id="${slot.id}">&#128196;</span>
       <span class="slot-arrow next-ir-btn" data-slot-id="${slot.id}">&#9654;</span>`
      : '';

  return `
    <div class="slot-meta">
      <div class="slot-title-row" style="display:flex;align-items:center;justify-content:space-between;gap:8px;">
        <h3 class="slot-title" style="margin:0;">${mod?.title ?? slot.type}</h3>
        <div class="rack-slot-arrows-container">${arrowsHTML}</div>
      </div>
      <span class="slot-desc" style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:120px;" title="${slot.name}">${slot.name}</span>
      <span class="slot-toolabr">
        <span class="slot-lane-selector">
          <button class="lane-opt ${lane === 'left' ? 'active' : ''}" data-lane="left" data-slot-id="${slot.id}">L</button>
          <button class="lane-opt ${lane === 'serial' ? 'active' : ''}" data-lane="serial" data-slot-id="${slot.id}">S</button>
          <button class="lane-opt ${lane === 'right' ? 'active' : ''}" data-lane="right" data-slot-id="${slot.id}">R</button>
        </span>
        <span class="slot-pan-control" data-slot-id="${slot.id}" title="Drag left/right to Pan">${panText}</span>
      </span>
    </div>
    ${!isHalfSlot ? `<div class="slot-controls">${inlineKnobsHTML}</div>` : ''}`;
}

function createSlotElement(slot, index) {
  const slotEl = document.createElement('div');
  slotEl.classList.add('rack-slot');
  slotEl.setAttribute('data-effect', slot.type);
  slotEl.setAttribute('data-id', slot.id);
  slotEl.setAttribute('data-index', index);
  if (slot.bypassed) slotEl.classList.add('bypassed');
  if (state.selected_slot_id === slot.id) slotEl.classList.add('selected');
  if (slot.lane !== 'serial') slotEl.classList.add('half-slot');

  slotEl.innerHTML = `
    <canvas class="slot-visualizer-canvas" data-slot-id="${slot.id}" style="position:absolute;top:0;left:0;width:100%;height:100%;pointer-events:none;z-index:0;opacity:0.18;border-radius:8px;"></canvas>
    <div style="position:relative;z-index:1;width:100%;height:100%;display:flex;align-items:center;justify-content:space-between;">
      ${createSlotHTML(slot)}
    </div>`;
  return slotEl;
}

// --- Main rack renderer ---

export function renderRack() {
  closeFileBrowserDropdown();
  const rackContainer = document.getElementById('rack-stack');
  rackContainer.innerHTML = '';

  let i = 0;
  while (i < state.routing_order.length) {
    const slot = state.routing_order[i];

    if (slot.lane === 'serial') {
      rackContainer.appendChild(createSlotElement(slot, i));
      i++;
    } else {
      // Parallel: pack left/right slots into a split row
      const slot1 = slot;
      const nextSlot = state.routing_order[i + 1];
      let leftItem = null, rightItem = null, advanced = 1;

      if (slot1.lane === 'left') {
        leftItem = { slot: slot1, index: i };
        if (nextSlot?.lane === 'right') { rightItem = { slot: nextSlot, index: i + 1 }; advanced = 2; }
      } else if (slot1.lane === 'right') {
        rightItem = { slot: slot1, index: i };
        if (nextSlot?.lane === 'left') { leftItem = { slot: nextSlot, index: i + 1 }; advanced = 2; }
      }

      const splitRowEl = document.createElement('div');
      splitRowEl.classList.add('parallel-split-row');

      ['left', 'right'].forEach((col, ci) => {
        const colEl = document.createElement('div');
        colEl.classList.add('parallel-column', `${col}-column`);
        const item = ci === 0 ? leftItem : rightItem;
        if (item) {
          colEl.appendChild(createSlotElement(item.slot, item.index));
        } else {
          const emptyCard = document.createElement('div');
          emptyCard.classList.add('empty-parallel-slot', 'btn-add-inline-trigger');
          emptyCard.setAttribute('data-target-lane', col);
          emptyCard.setAttribute('data-insert-index', ci === 0 ? i : (leftItem?.index ?? i) + 1);
          emptyCard.innerHTML = `<span style="font-size:14px;margin-bottom:2px;">+</span> Add ${col === 'left' ? 'Left' : 'Right'} Slot`;
          colEl.appendChild(emptyCard);
        }
        splitRowEl.appendChild(colEl);
      });

      rackContainer.appendChild(splitRowEl);
      i += advanced;
    }
  }

  bindRackEvents();
}

// --- Rack event bindings ---

export function bindRackEvents() {
  const rackContainer = document.getElementById('rack-stack');

  // 1. Select slot (click) and double-click bypass toggle
  rackContainer.querySelectorAll('.rack-slot').forEach(slotEl => {
    const slotId = slotEl.getAttribute('data-id');
    const slot = state.routing_order.find(s => s.id === slotId);

    slotEl.addEventListener('click', e => {
      if (e.target.closest('.knob-widget') || e.target.closest('.slot-pan-control') ||
        e.target.closest('.slot-arrows-container') || e.target.closest('.lane-opt')) return;
      state.selected_slot_id = slotId;
      rackContainer.querySelectorAll('.rack-slot').forEach(s => s.classList.remove('selected'));
      slotEl.classList.add('selected');
      renderInspector();
    });

    slotEl.addEventListener('dblclick', e => {
      if (e.target.closest('.knob-widget') || e.target.closest('.slot-pan-control') ||
        e.target.closest('.slot-arrows-container') || e.target.closest('.lane-opt')) return;
      if (!slot) return;
      slot.bypassed = !slot.bypassed;
      updateSlotBypass(slotId, slot.bypassed);
      syncSlotsToRust();
    });
  });

  // 1a. Lane switch
  rackContainer.querySelectorAll('.lane-opt').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const slotId = btn.getAttribute('data-slot-id');
      const targetLane = btn.getAttribute('data-lane');
      const slot = state.routing_order.find(s => s.id === slotId);
      if (slot) { slot.lane = targetLane; renderRack(); syncSlotsToRust(); }
    });
  });

  // 1b. Panning drag
  rackContainer.querySelectorAll('.slot-pan-control').forEach(ctrl => {
    const slotId = ctrl.getAttribute('data-slot-id');
    const slot = state.routing_order.find(s => s.id === slotId);
    if (!slot) return;
    let startX = 0, startVal = 0;

    function onMouseMove(e) {
      const newVal = Math.max(-1, Math.min(1, Math.round((startVal + (e.clientX - startX) * 0.008) * 100) / 100));
      slot.pan = newVal;
      updateSlotPan(slotId, newVal);
    }
    function onMouseUp() {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      ctrl.style.cursor = 'ew-resize';
      syncSlotsToRust();
    }

    ctrl.addEventListener('mousedown', e => {
      startX = e.clientX;
      startVal = slot.pan ?? 0;
      ctrl.style.cursor = 'grabbing';
      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    });
    ctrl.addEventListener('dblclick', e => {
      e.stopPropagation();
      slot.pan = 0;
      updateSlotPan(slotId, 0);
      syncSlotsToRust();
    });
  });

  // 2. Drag-to-reorder slots
  rackContainer.querySelectorAll('.rack-slot').forEach(slotEl => {
    const slotId = slotEl.getAttribute('data-id');
    const addTrigger = document.getElementById('btn-add-trigger');

    slotEl.addEventListener('mouseenter', () => {
      if (addTrigger && !addTrigger.classList.contains('delete-zone')) {
        addTrigger.textContent = 'drag slots to reorder';
      }
    });
    slotEl.addEventListener('mouseleave', () => {
      if (addTrigger && !addTrigger.classList.contains('delete-zone')) {
        addTrigger.textContent = '+ Add Slot';
      }
    });

    slotEl.addEventListener('mousedown', e => {
      if (e.target.closest('.knob-widget') || e.target.closest('.slot-pan-control') ||
        e.target.closest('.slot-arrows-container') || e.target.closest('.lane-opt')) return;
      e.preventDefault();

      let hasDragged = false;
      const startY = e.clientY;

      function onMouseMove(moveEvent) {
        if (Math.abs(moveEvent.clientY - startY) > 4 && !hasDragged) {
          hasDragged = true;
          slotEl.classList.add('dragging');
          rackContainer.classList.add('drag-active');
          if (addTrigger) {
            addTrigger.innerHTML = '<span class="delete-icon">&#128465;</span> Drop here to Delete';
            addTrigger.classList.add('delete-zone');
          }
        }
        if (!hasDragged) return;

        const slots = Array.from(rackContainer.querySelectorAll('.rack-slot'));
        const curIdx = state.routing_order.findIndex(s => s.id === slotId);
        if (curIdx === -1) return;

        if (addTrigger) {
          const addRect = addTrigger.getBoundingClientRect();
          addTrigger.classList.toggle('delete-zone-hover',
            moveEvent.clientX >= addRect.left && moveEvent.clientX <= addRect.right &&
            moveEvent.clientY >= addRect.top && moveEvent.clientY <= addRect.bottom);
        }

        for (let i = 0; i < slots.length; i++) {
          const s = slots[i];
          const sId = s.getAttribute('data-id');
          if (sId === slotId) continue;
          const rect = s.getBoundingClientRect();
          const mid = rect.top + rect.height / 2;
          if (i < curIdx && moveEvent.clientY < mid) {
            const [dragged] = state.routing_order.splice(curIdx, 1);
            state.routing_order.splice(i, 0, dragged);
            renderRack();
            const moved = rackContainer.querySelector(`.rack-slot[data-id="${slotId}"]`);
            if (moved) { moved.classList.add('dragging', 'drag-hover-target'); }
            break;
          } else if (i > curIdx && moveEvent.clientY > mid) {
            const [dragged] = state.routing_order.splice(curIdx, 1);
            state.routing_order.splice(i, 0, dragged);
            renderRack();
            const moved = rackContainer.querySelector(`.rack-slot[data-id="${slotId}"]`);
            if (moved) { moved.classList.add('dragging', 'drag-hover-target'); }
            break;
          }
        }
      }

      function onMouseUp(upEvent) {
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
        if (addTrigger) {
          addTrigger.innerHTML = '+ Add Slot';
          addTrigger.classList.remove('delete-zone', 'delete-zone-hover');
        }

        if (hasDragged) {
          rackContainer.classList.remove('drag-active');
          slotEl.classList.remove('dragging');

          // Check if dropped on delete zone
          if (addTrigger) {
            const addRect = addTrigger.getBoundingClientRect();
            if (upEvent.clientX >= addRect.left && upEvent.clientX <= addRect.right &&
              upEvent.clientY >= addRect.top && upEvent.clientY <= addRect.bottom) {
              onSlotDeleted(slotId);
              state.routing_order = state.routing_order.filter(s => s.id !== slotId);
              if (state.selected_slot_id === slotId) {
                state.selected_slot_id = state.routing_order[0]?.id ?? '';
              }
            }
          }
          renderRack();
          renderInspector();
          syncSlotsToRust();
        }
      }

      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    });
  });

  // 3. Inline "+ Add Slot" in parallel columns
  rackContainer.querySelectorAll('.btn-add-inline-trigger').forEach(trigger => {
    trigger.addEventListener('click', e => {
      e.stopPropagation();
      const targetLane = trigger.getAttribute('data-target-lane');
      const insertIndex = parseInt(trigger.getAttribute('data-insert-index'));

      const availableToAdd = [
        { id: 'pitch', label: 'Pitch Shifter' },
        { id: 'amp', label: 'Amp' },
        { id: 'cab', label: 'Cab' },
        { id: 'delay', label: 'Delay' },
        { id: 'verb', label: 'Reverb' },
        { id: 'shimmer', label: 'Cosmos' },
        { id: 'gate', label: 'Gate' },
        { id: 'error', label: 'Error (Bitcrush)' },
        { id: 'od', label: 'OD (Overdrive)' },
        { id: 'eq', label: 'EQ (Parametric)' },
      ];

      const dropdown = document.createElement('div');
      dropdown.classList.add('add-dropdown', 'show');
      dropdown.style.cssText = 'bottom:auto;top:100%;position:absolute;left:0;right:0;';

      availableToAdd.forEach(item => {
        const btn = document.createElement('button');
        btn.classList.add('add-dropdown-item');
        btn.textContent = `+ ${item.label}`;
        btn.addEventListener('click', ev => {
          ev.stopPropagation();
          const slotId = `${item.id}_${Date.now()}`;
          const newSlot = {
            id: slotId, type: item.id,
            name: item.id === 'amp' ? 'Empty (No Model)' : item.id === 'cab' ? 'Empty (No IR)' : (modulesData[item.id]?.desc ?? item.id),
            path: null, bypassed: false, pan: 0.0, lane: targetLane,
            params: getDefaultSlotParams(item.id),
          };
          state.routing_order.splice(insertIndex, 0, newSlot);
          state.selected_slot_id = slotId;
          dropdown.remove();
          renderRack();
          renderInspector();
          syncSlotsToRust();
        });
        dropdown.appendChild(btn);
      });

      trigger.appendChild(dropdown);
    });
  });

  // 4. Knob reassignment triggers
  rackContainer.querySelectorAll('.knob-reassign-trigger').forEach(trigger => {
    trigger.addEventListener('click', e => {
      e.stopPropagation();
      document.querySelectorAll('.reassign-popover').forEach(p => p.remove());
      const slotId = trigger.getAttribute('data-slot-id');
      const slot = state.routing_order.find(s => s.id === slotId);
      if (!slot) return;
      const knobIdx = parseInt(trigger.getAttribute('data-knob-index'));
      const widget = trigger.closest('.knob-widget');
      const availableParams = modulesData[slot.type]?.knobs ?? [];

      const popover = document.createElement('div');
      popover.classList.add('reassign-popover');
      availableParams.forEach(paramId => {
        const btn = document.createElement('button');
        btn.classList.add('reassign-option');
        btn.textContent = paramId.replace(/_/g, ' ');
        btn.addEventListener('click', () => {
          state.slot_knobs[slot.type][knobIdx] = paramId;
          renderRack();
          popover.remove();
        });
        popover.appendChild(btn);
      });
      widget.appendChild(popover);
    });
  });

  document.addEventListener('click', () => {
    document.querySelectorAll('.reassign-popover').forEach(p => p.remove());
  });

  // 5. File browse arrows
  rackContainer.querySelectorAll('.browse-nam-btn').forEach(btn => {
    btn.addEventListener('click', e => { e.stopPropagation(); browseNamFile(btn.getAttribute('data-slot-id')); });
  });
  rackContainer.querySelectorAll('.browse-cab-btn').forEach(btn => {
    btn.addEventListener('click', e => { e.stopPropagation(); browseCabFile(btn.getAttribute('data-slot-id')); });
  });
  rackContainer.querySelectorAll('.prev-model-btn').forEach(btn => {
    btn.addEventListener('click', e => { e.stopPropagation(); prevModel(btn.getAttribute('data-slot-id')); });
  });
  rackContainer.querySelectorAll('.next-model-btn').forEach(btn => {
    btn.addEventListener('click', e => { e.stopPropagation(); nextModel(btn.getAttribute('data-slot-id')); });
  });
  rackContainer.querySelectorAll('.prev-ir-btn').forEach(btn => {
    btn.addEventListener('click', e => { e.stopPropagation(); prevIR(btn.getAttribute('data-slot-id')); });
  });
  rackContainer.querySelectorAll('.next-ir-btn').forEach(btn => {
    btn.addEventListener('click', e => { e.stopPropagation(); nextIR(btn.getAttribute('data-slot-id')); });
  });

  bindKnobDragging(rackContainer);
}

function getDefaultSlotParams(type) {
  const defaults = {
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
  return { ...(defaults[type] ?? {}) };
}
