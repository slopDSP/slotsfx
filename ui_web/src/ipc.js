// --- IPC bridge: JS ↔ Rust via WebView messaging ---

import { state } from './state.js';

// Sends a typed JSON message to Rust over the WebView IPC channel.
export function sendIPCMessage(type, payload) {
  const msg = { type, ...payload };
  if (window.chrome && window.chrome.webview && window.chrome.webview.postMessage) {
    try {
      window.chrome.webview.postMessage(JSON.stringify(msg));
    } catch (e) {
      console.error('[IPC] Failed to post message:', e);
    }
  }
}

// Sends the full current slot config back to Rust so it persists the state.
export function syncSlotsToRust() {
  const slots = state.routing_order.map(slot => {
    const slotClone = { ...slot };
    if (slotClone.params && typeof slotClone.params === 'object') {
      const cleanParams = {};
      for (const [key, val] of Object.entries(slotClone.params)) {
        if (val !== undefined && val !== null) {
          cleanParams[key] = typeof val === 'boolean' ? (val ? 1.0 : 0.0) : val;
        }
      }
      slotClone.params = cleanParams;
    }
    return slotClone;
  });
  sendIPCMessage('update_slots', { slots });
}
