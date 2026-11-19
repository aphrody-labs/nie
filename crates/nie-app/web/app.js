// The page of the nie browser app. Every decoder, the VFS mount and the wiki are the Rust code of
// nie-wasm; this file only wires them to the DOM.
import init, { GameMount, ModelViewer, g4tx_to_png, model_to_glb } from './nie_wasm.js';

const $ = (id) => document.getElementById(id);
const PAGE = 500;

const state = { mount: null, dir: '', files: [], shown: 0, viewer: null, loop: 0, wasm: null };

const caps = {
  secure: window.isSecureContext,
  gpu: 'gpu' in navigator,
  directory: 'showDirectoryPicker' in window,
  opfs: !!(navigator.storage && navigator.storage.getDirectory),
};

function say(text) {
  $('live').textContent = text;
}

function fail(target, error) {
  const el = $(target);
  el.textContent = String(error && error.message ? error.message : error);
  el.hidden = false;
}

function bytes(n) {
  return Number(n).toLocaleString('en-US');
}

function showCapabilities() {
  const flag = (ok, name) => `${name}: ${ok ? 'yes' : 'no'}`;
  $('caps').textContent = [
    flag(caps.gpu, 'WebGPU'),
    flag(caps.directory, 'folder access'),
    flag(caps.secure, 'secure context'),
  ].join(' | ');
  $('pick-dir').disabled = !caps.directory;
  if (!caps.directory) {
    $('pick-dir').title = 'This browser has no File System Access API: connect to a server instead.';
  }
}

// ---------------------------------------------------------------------------------------------
// Remembered folder: a directory handle can be stored in IndexedDB, not in OPFS.
// ---------------------------------------------------------------------------------------------

function store(mode, run) {
  return new Promise((resolve) => {
    const open = indexedDB.open('nie', 1);
    open.onupgradeneeded = () => open.result.createObjectStore('handles');
    open.onerror = () => resolve(undefined);
    open.onsuccess = () => {
      const tx = open.result.transaction('handles', mode);
      const request = run(tx.objectStore('handles'));
      tx.oncomplete = () => resolve(request && request.result);
      tx.onerror = () => resolve(undefined);
    };
  });
}

const remember = (handle) => store('readwrite', (s) => s.put(handle, 'game'));
const recall = () => store('readonly', (s) => s.get('game'));

// ---------------------------------------------------------------------------------------------
// Mounting
// ---------------------------------------------------------------------------------------------

async function mounted(mount) {
  state.mount = mount;
  const summary = JSON.parse(mount.summary());
  $('mount').hidden = true;
  $('tabs').hidden = false;
  $('summary').textContent = `${bytes(summary.entries)} entries in ${bytes(summary.packs)} packs, read through ${summary.kind === 'directory' ? 'your folder' : 'the server'}.`;
  showView('explorer');
  openDir('');
  say(`Mounted: ${summary.entries} entries.`);
  return summary;
}

async function mountDirectory(handle) {
  $('mount-error').hidden = true;
  try {
    const summary = await mounted(await GameMount.mount_directory(handle));
    await remember(handle);
    return summary;
  } catch (error) {
    fail('mount-error', error);
    throw error;
  }
}

async function mountHttp(base, token) {
  $('mount-error').hidden = true;
  try {
    return await mounted(await GameMount.mount_http(base || '', token || undefined));
  } catch (error) {
    fail('mount-error', error);
    throw error;
  }
}

$('pick-dir').addEventListener('click', async () => {
  try {
    await mountDirectory(await window.showDirectoryPicker({ mode: 'read', id: 'nie-game' }));
  } catch (error) {
    if (error && error.name !== 'AbortError') fail('mount-error', error);
  }
});

$('reopen').addEventListener('click', async () => {
  const handle = await recall();
  if (!handle) return;
  if ((await handle.requestPermission({ mode: 'read' })) !== 'granted') {
    fail('mount-error', 'Read access to the folder was not granted.');
    return;
  }
  try {
    await mountDirectory(handle);
  } catch (error) {
    fail('mount-error', error);
  }
});

$('server').addEventListener('submit', async (event) => {
  event.preventDefault();
  try {
    await mountHttp($('server-url').value.trim(), $('server-token').value.trim());
  } catch (error) {
    fail('mount-error', error);
  }
});

// ---------------------------------------------------------------------------------------------
// Explorer
// ---------------------------------------------------------------------------------------------

function crumbs(dir) {
  const nav = $('crumbs');
  nav.replaceChildren();
  const parts = dir ? dir.split('/') : [];
  const add = (label, target) => {
    const b = document.createElement('button');
    b.type = 'button';
    b.textContent = label;
    b.addEventListener('click', () => openDir(target));
    nav.append(b);
  };
  add('VFS root', '');
  parts.forEach((part, i) => add(part, parts.slice(0, i + 1).join('/')));
}

function row(label, size, onClick) {
  const tr = document.createElement('tr');
  const name = document.createElement('td');
  const b = document.createElement('button');
  b.type = 'button';
  b.textContent = label;
  b.addEventListener('click', onClick);
  name.append(b);
  const n = document.createElement('td');
  n.className = 'num';
  n.textContent = size === null ? '' : bytes(size);
  tr.append(name, n);
  return tr;
}

function openDir(dir) {
  state.dir = dir;
  const listing = JSON.parse(state.mount.list(dir));
  crumbs(dir);
  const body = $('listing').tBodies[0];
  body.replaceChildren();
  for (const d of listing.dirs) {
    const target = dir ? `${dir}/${d.name}` : d.name;
    body.append(row(`${d.name}/ (${bytes(d.files)})`, null, () => openDir(target)));
  }
  state.files = listing.files;
  state.shown = 0;
  moreFiles();
  say(`${listing.dirs.length} folders, ${listing.files.length} files.`);
}

function moreFiles() {
  const body = $('listing').tBodies[0];
  const next = state.files.slice(state.shown, state.shown + PAGE);
  for (const f of next) body.append(row(f.name, f.size, () => openFile(f.path)));
  state.shown += next.length;
  $('more').hidden = state.shown >= state.files.length;
}

$('more').addEventListener('click', moreFiles);

$('search').addEventListener('submit', (event) => {
  event.preventDefault();
  const q = $('search-q').value.trim();
  if (!q) return openDir(state.dir);
  const found = JSON.parse(state.mount.find(q, 200));
  const body = $('listing').tBodies[0];
  body.replaceChildren();
  state.files = [];
  state.shown = 0;
  $('more').hidden = true;
  for (const item of found.items) body.append(row(item.path, item.size, () => openFile(item.path)));
  $('crumbs').replaceChildren();
  say(`${found.count} matches, ${found.items.length} shown.`);
});

let image = null;

async function openFile(path) {
  $('detail').hidden = false;
  $('detail-title').textContent = path;
  $('detail-meta').textContent = 'Reading...';
  $('detail-text').textContent = '';
  $('detail-image').hidden = true;
  $('open-model').hidden = true;
  if (image) URL.revokeObjectURL(image);
  image = null;
  try {
    const info = JSON.parse(await state.mount.describe(path));
    $('detail-meta').textContent = `${info.format}, ${bytes(info.size)} bytes`;
    $('detail-text').textContent = info.summary || '(no structured summary for this format)';
    if (info.format === 'G4TX') {
      const png = g4tx_to_png(await state.mount.read(path));
      image = URL.createObjectURL(new Blob([png], { type: 'image/png' }));
      $('detail-image').src = image;
      $('detail-image').alt = `Texture ${path}`;
      $('detail-image').hidden = false;
    }
    if (path.toLowerCase().endsWith('.g4md')) {
      $('open-model').hidden = false;
      $('open-model').onclick = () => openModel(path);
    }
  } catch (error) {
    $('detail-meta').textContent = `Cannot read this entry: ${error.message || error}`;
  }
  $('detail').scrollIntoView({ block: 'nearest' });
}

// ---------------------------------------------------------------------------------------------
// Wiki
// ---------------------------------------------------------------------------------------------

async function wikiPage(target) {
  const frame = $('wiki-frame');
  const answer = JSON.parse(await state.mount.wiki_page(target));
  frame.srcdoc = answer.html;
  await new Promise((resolve) => frame.addEventListener('load', resolve, { once: true }));
  const doc = frame.contentDocument;
  doc.addEventListener('click', (event) => {
    const link = event.target.closest && event.target.closest('a[href^="/wiki"]');
    if (link) {
      event.preventDefault();
      wikiPage(link.getAttribute('href'));
    }
  });
  doc.addEventListener('submit', (event) => {
    const form = event.target;
    const action = form.getAttribute('action') || target.split('?')[0];
    event.preventDefault();
    const query = new URLSearchParams(new FormData(form)).toString();
    wikiPage(query ? `${action}?${query}` : action);
  });
  say(`Wiki page ${target}: status ${answer.status}.`);
  return answer.status;
}

// ---------------------------------------------------------------------------------------------
// Model viewer
// ---------------------------------------------------------------------------------------------

async function openModel(path) {
  showView('viewer');
  const status = $('viewer-status');
  try {
    status.textContent = 'Assembling the model...';
    const geometry = path.replace(/\.g4md$/i, '.g4mg');
    const glb = model_to_glb(await state.mount.read(path), await state.mount.read(geometry));
    if (!state.viewer) {
      state.viewer = await ModelViewer.create($('canvas'));
      attachControls($('canvas'));
    }
    state.viewer.load_glb(glb);
    status.textContent = `${path}: ${JSON.parse(state.viewer.adapter()).backend}`;
    startLoop();
  } catch (error) {
    status.textContent = `The model cannot be shown: ${error.message || error}`;
  }
}

const camera = { yaw: 0.6, pitch: 0.3, distance: 2.5 };

function attachControls(canvas) {
  let last = null;
  canvas.addEventListener('pointerdown', (e) => { last = [e.clientX, e.clientY]; canvas.setPointerCapture(e.pointerId); });
  canvas.addEventListener('pointerup', () => { last = null; });
  canvas.addEventListener('pointermove', (e) => {
    if (!last) return;
    camera.yaw -= (e.clientX - last[0]) * 0.01;
    camera.pitch = Math.max(-1.4, Math.min(1.4, camera.pitch + (e.clientY - last[1]) * 0.01));
    last = [e.clientX, e.clientY];
  });
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    camera.distance = Math.max(0.5, Math.min(20, camera.distance * (1 + Math.sign(e.deltaY) * 0.1)));
  }, { passive: false });
  canvas.addEventListener('keydown', (e) => {
    const step = 0.1;
    if (e.key === 'ArrowLeft') camera.yaw += step;
    if (e.key === 'ArrowRight') camera.yaw -= step;
    if (e.key === 'ArrowUp') camera.pitch = Math.min(1.4, camera.pitch + step);
    if (e.key === 'ArrowDown') camera.pitch = Math.max(-1.4, camera.pitch - step);
    if (e.key === '+') camera.distance = Math.max(0.5, camera.distance * 0.9);
    if (e.key === '-') camera.distance = Math.min(20, camera.distance * 1.1);
  });
}

function startLoop() {
  cancelAnimationFrame(state.loop);
  const frame = () => {
    if (!state.viewer || $('viewer').hidden) return;
    try {
      state.viewer.orbit(camera.yaw, camera.pitch, camera.distance);
      state.viewer.render();
      state.loop = requestAnimationFrame(frame);
    } catch (error) {
      $('viewer-status').textContent = `The viewer stopped: ${error.message || error}`;
    }
  };
  state.loop = requestAnimationFrame(frame);
}

// ---------------------------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------------------------

function showView(view) {
  for (const name of ['explorer', 'wiki', 'viewer']) $(name).hidden = name !== view;
  for (const b of $('tabs').querySelectorAll('button')) b.setAttribute('aria-pressed', String(b.dataset.view === view));
  if (view === 'wiki' && state.mount && !$('wiki-frame').srcdoc) wikiPage('/wiki');
  if (view === 'viewer' && state.viewer) startLoop();
}

$('tabs').addEventListener('click', (event) => {
  const view = event.target.dataset && event.target.dataset.view;
  if (view) showView(view);
});

// ---------------------------------------------------------------------------------------------

async function main() {
  showCapabilities();
  state.wasm = await init();
  const handle = await recall();
  if (handle && caps.directory) {
    $('reopen').textContent = `Reopen ${handle.name}`;
    $('reopen').hidden = false;
  }
  // The same-origin server, when it holds an installation, is offered as the default.
  const health = await fetch('/health').then((r) => (r.ok ? r.json() : null)).catch(() => null);
  if (health && health.vfsReady) $('server-url').placeholder = `this server (${health.entries} entries)`;
  // For automation and scripting: the mount functions and the state, nothing more.
  window.nieApp = { mountDirectory, mountHttp, openDir, openFile, wikiPage, showView, state, caps };
  document.documentElement.dataset.ready = 'true';
}

main().catch((error) => fail('mount-error', error));
