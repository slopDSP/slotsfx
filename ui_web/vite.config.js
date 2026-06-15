import { defineConfig } from 'vite';

export default defineConfig({
  // Use relative paths so the build works when loaded from a file:// URL
  // (which is how the webview loads it via file:///{...}/dist/index.html).
  // Without this, Vite emits absolute paths like /assets/index-XXX.js which
  // resolve to file:///C:/assets/index-XXX.js under file:// — doesn't exist.
  base: './',
});
