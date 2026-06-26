/**
 * Minimal JS glue — the ONLY JavaScript in the project.
 * Bridges browser APIs that WASM cannot access directly:
 *   - Canvas setup + WebGPU availability check
 *   - Input event forwarding (DOM -> WASM)
 *   - requestAnimationFrame loop
 *   - DPI-aware canvas sizing
 *
 * GPU initialization is handled entirely in Rust via wgpu.
 */

import init, { CadApp } from '/pkg/physical_web.js';

async function main() {
  // 1. Initialize WASM
  await init();

  // 2. Canvas setup — DPI-aware sizing
  const canvas = document.getElementById('canvas');
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(window.innerWidth * dpr);
  canvas.height = Math.round(window.innerHeight * dpr);

  // 3. WebGPU availability check
  if (!navigator.gpu) {
    console.warn('[engine] WebGPU not available, using WebGL2 fallback');
  }

  // 4. Create app — Rust initializes WebGPU, compiles shaders, builds pipeline
  const app = await CadApp.new(canvas, dpr);

  // 5. Input forwarding -> WASM
  const keys = new Set();

  document.addEventListener('keydown', (e) => {
    // Prevent browser defaults for keys we handle
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyK') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyZ') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyY') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyD') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyA') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyS') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyP') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyC') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyX') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyF') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyM') e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyN') e.preventDefault();
    if (e.code === 'F1') e.preventDefault();
    if (e.code === 'F3') e.preventDefault();
    if (e.code === 'F5') e.preventDefault();
    if (e.code === 'Backspace') e.preventDefault();
    if (e.code.startsWith('Numpad') || e.code.startsWith('Digit')) {
      // Allow digits in text inputs but prevent navigation shortcuts
    }
    if (!keys.has(e.code)) {
      keys.add(e.code);
      app.key_down(e.code);
    }
  });

  document.addEventListener('keyup', (e) => {
    keys.delete(e.code);
    app.key_up(e.code);
  });

  canvas.addEventListener('mousemove', (e) => {
    app.mouse_move(e.clientX, e.clientY, e.movementX, e.movementY);
  });

  canvas.addEventListener('mousedown', (e) => {
    app.mouse_down(e.button);
  });

  canvas.addEventListener('mouseup', (e) => {
    app.mouse_up(e.button);
  });

  canvas.addEventListener('wheel', (e) => {
    app.mouse_wheel(e.deltaY);
    e.preventDefault();
  }, { passive: false });

  // Prevent context menu on right-click (we use right-click for orbit)
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());

  canvas.addEventListener('touchstart', (e) => {
    for (const t of e.changedTouches) {
      app.touch_start(t.identifier, t.clientX, t.clientY);
    }
  });

  canvas.addEventListener('touchmove', (e) => {
    for (const t of e.changedTouches) {
      app.touch_move(t.identifier, t.clientX, t.clientY);
    }
    e.preventDefault();
  }, { passive: false });

  canvas.addEventListener('touchend', (e) => {
    for (const t of e.changedTouches) {
      app.touch_end(t.identifier);
    }
  });

  // 6. Resize handling — DPI-aware
  window.addEventListener('resize', () => {
    const currentDpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(window.innerWidth * currentDpr);
    canvas.height = Math.round(window.innerHeight * currentDpr);
    app.resize(canvas.width, canvas.height);
  });

  // 7. DPR change listener (drag between displays with different scaling)
  function watchDpr() {
    const mq = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    mq.addEventListener('change', () => {
      const newDpr = window.devicePixelRatio || 1;
      canvas.width = Math.round(window.innerWidth * newDpr);
      canvas.height = Math.round(window.innerHeight * newDpr);
      app.resize(canvas.width, canvas.height);
      watchDpr();
    }, { once: true });
  }
  watchDpr();

  // 8. Main loop
  function frame(timestamp) {
    const seconds = timestamp / 1000.0;
    app.frame(seconds);
    requestAnimationFrame(frame);
  }

  requestAnimationFrame(frame);
}

main().catch(console.error);
