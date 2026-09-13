/* ============================================================================
 * app.js — Printables Offline frontend
 * ============================================================================
 * Wires the UI to the real Tauri backend via window.__TAURI__.core.invoke.
 * Uses localStorage for theme/slicer prefs (po.theme, po.slicer) and syncs
 * with the backend via set_prefs on change.
 * ========================================================================== */

/* ============================ BRIDGE ============================ */
const Bridge = {
  async call(command, payload) {
    if (!window.__TAURI__) {
      console.warn('[bridge] Tauri not available, mocking:', command, payload);
      return { ok: true, command, payload };
    }
    return window.__TAURI__.core.invoke(command, { args: payload || {} });
  },
  async listen(event, handler) {
    if (!window.__TAURI__) {
      console.warn('[bridge] Tauri not available, cannot listen:', event);
      return () => {};
    }
    return window.__TAURI__.event.listen(event, handler);
  }
};

/* ============================ DATA ============================ */
const DISK_GB = 4.0;
const RECENT_CUTOFF_DAYS = 30;

let state = {
  view: 'library',
  detailId: null,
  search: '',
  filter: 'all',
  sort: 'added',
  gallery: 0,
  models: [],
  collections: [],
  activeCollectionId: null,
  totals: { models: 0, files: 0, bytes_used: 0, bytes_capacity: DISK_GB * 1024 * 1024 * 1024 },
  prefs: { theme: 'system', slicer_key: 'prusa', slicer_executable: null, library_folder: null, python_path: null },
  libraryConfigured: false
};

const SLICERS = {
  prusa: ['PrusaSlicer', '/Applications/PrusaSlicer.app/Contents/MacOS/PrusaSlicer'],
  orca: ['OrcaSlicer', '/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer'],
  bambu: ['Bambu Studio', '/Applications/BambuStudio.app/Contents/MacOS/BambuStudio'],
  cura: ['UltiMaker Cura', '/Applications/UltiMaker Cura.app/Contents/MacOS/UltiMaker-Cura']
};

/* ============================ HELPERS ============================ */
const $  = (s, el) => (el || document).querySelector(s);
const $$ = (s, el) => Array.from((el || document).querySelectorAll(s));
const esc = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];

function fmtDate(iso, short) {
  if (!iso) return 'unknown';
  // Handle full ISO timestamps like "2026-08-17T00:19:17Z" by splitting on T first
  const datePart = iso.split('T')[0];
  const [y, m, d] = datePart.split('-').map(Number);
  if (isNaN(y) || isNaN(m) || isNaN(d)) return iso;
  return short ? `${MONTHS[m-1]} ${d}` : `${MONTHS[m-1]} ${d}, ${y}`;
}

function fmtMB(mb) {
  if (mb >= 1024) return (mb/1024).toFixed(1) + ' GB';
  if (mb >= 100) return Math.round(mb) + ' MB';
  return (Math.round(mb*10)/10) + ' MB';
}

function fmtBytes(bytes) {
  const gb = bytes / (1024 * 1024 * 1024);
  return gb.toFixed(1) + ' GB';
}

function extOf(n) { return n.split('.').pop().toLowerCase(); }
function extClass(e) { return {'3mf':'three-mf', stl:'stl', obj:'obj', gcode:'gcode'}[e] || ''; }

function modelSize(m) {
  return (m.files || []).reduce((a, f) => a + (f.mb || 0), 0);
}

function fileCount() {
  return state.models.reduce((a, m) => a + (m.files || []).length, 0);
}

function libTotalMB() {
  return state.models.reduce((a, m) => a + modelSize(m), 0);
}

function extCounts(m) {
  const c = {};
  (m.files || []).forEach(f => { const e = extOf(f.n); c[e] = (c[e]||0) + 1; });
  return c;
}

function slugFromSource(src) {
  return (src.split('/model/')[1] || 'local-import').split(/[?#]/)[0];
}

function localPath(m) {
  const lib = state.prefs.library_folder || '~/Printables Library';
  return `${lib}/${slugFromSource(m.source)}/`;
}

function titleCase(s) {
  return s.replace(/\b\w/g, c => c.toUpperCase());
}

function isRecent(iso) {
  if (!iso) return false;
  const cutoff = new Date();
  cutoff.setDate(cutoff.getDate() - RECENT_CUTOFF_DAYS);
  const d = new Date(iso);
  return d >= cutoff;
}

/* ============================ ICONS ============================ */
const ICONS = {
  search: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>',
  sun: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>',
  moon: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>',
  monitor: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>',
  back: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>',
  download: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>',
  layers: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2 2 7l10 5 10-5-10-5z"/><path d="m2 17 10 5 10-5"/><path d="m2 12 10 5 10-5"/></svg>',
  check: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>',
  pencil: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>',
  trash: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>'
};
const GHOST = '<svg width="52" height="52" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M24 5 42 14.5v19L24 43 6 33.5v-19L24 5z"/><path d="M6 14.5 24 24l18-9.5M24 24v19"/></svg>';

/* ============================ THEME ============================ */
const THEME_KEY = 'po.theme';
const SLICER_KEY = 'po.slicer';

function resolvedTheme(pref) {
  return pref === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : pref;
}

function applyTheme(pref, silent) {
  state.prefs.theme = pref;
  try { localStorage.setItem(THEME_KEY, pref); } catch(e) {}
  document.documentElement.dataset.theme = resolvedTheme(pref);
  updateThemeUI();
  if (!silent) {
    Bridge.call('set_prefs', { theme: pref }).catch(() => {});
    toast(`Theme set to ${titleCase(pref)}`, 'synced to backend');
  }
}

function themeIcon() {
  return state.prefs.theme === 'light' ? ICONS.sun : state.prefs.theme === 'dark' ? ICONS.moon : ICONS.monitor;
}

function updateThemeUI() {
  $$('#theme-seg button').forEach(b => b.classList.toggle('is-active', b.dataset.pref === state.prefs.theme));
  const t = $('#btn-theme');
  if (t) { t.innerHTML = themeIcon(); t.title = `Theme: ${titleCase(state.prefs.theme)} — click to cycle`; }
}

function cycleTheme() {
  const order = ['light', 'dark', 'system'];
  applyTheme(order[(order.indexOf(state.prefs.theme) + 1) % order.length]);
}

matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (state.prefs.theme === 'system') applyTheme('system', true);
});

/* ============================ TOASTS ============================ */
function toast(title, sub) {
  const wrap = $('#toasts');
  const t = document.createElement('div');
  t.className = 'toast';
  t.innerHTML = `<div>${esc(title)}</div>${sub ? `<div class="tsub">${esc(sub)}</div>` : ''}`;
  wrap.appendChild(t);
  while (wrap.children.length > 3) wrap.firstChild.remove();
  setTimeout(() => {
    t.style.opacity = '0';
    t.style.transform = 'translateY(6px)';
    setTimeout(() => t.remove(), 320);
  }, 3400);
}

/* ============================ TOPBAR ============================ */
function renderTopbar() {
  const topbar = $('#topbar'), ctx = $('#topbar-context'), tools = $('#topbar-tools');
  topbar.dataset.view = state.view;
  let ctxHTML = '', toolsHTML = '';

  if (state.view === 'library') {
    const coll = activeCollection();
    if (coll) {
      ctxHTML = `
        <button class="topbar-back" id="topbar-back-collection">${ICONS.back}<span>Local Library</span></button>
        <span class="topbar-sep">/</span>
        <h1 class="topbar-title">${esc(coll.name)}</h1>
        <span class="topbar-sub" id="lib-count"></span>`;
    } else {
      ctxHTML = `<h1 class="topbar-title">Local Library</h1><span class="topbar-sub" id="lib-count"></span>`;
    }
    toolsHTML = `
      <label class="searchbox">${ICONS.search}
        <input id="search-input" type="text" placeholder="Search models, creators, tags…" value="${esc(state.search)}" autocomplete="off" spellcheck="false" />
        <kbd>/</kbd>
      </label>
      <label class="sortbox"><span>Sort</span>
        <select id="sort-select">
          <option value="added">Recently added</option>
          <option value="name">Name A–Z</option>
          <option value="size">Size (largest)</option>
        </select>
      </label>`;
    if (coll) {
      toolsHTML += `
        <button class="btn btn-secondary btn-sm" id="btn-rename-collection" data-id="${coll.id}">Rename</button>
        <button class="btn btn-ghost btn-danger btn-sm" id="btn-delete-collection" data-id="${coll.id}">Delete Collection</button>`;
    }
  } else if (state.view === 'detail') {
    const m = state.models.find(x => x.id === state.detailId);
    ctxHTML = `
      <button class="topbar-back" id="topbar-back">${ICONS.back}<span>Library</span></button>
      <span class="topbar-sep">/</span>
      <h1 class="topbar-title">${esc(m ? m.name : '')}</h1>`;
  } else if (state.view === 'clone') {
    ctxHTML = `<h1 class="topbar-title">Download / Clone</h1>`;
  } else {
    ctxHTML = `<h1 class="topbar-title">Preferences</h1>`;
  }

  toolsHTML += `<button class="btn-icon" id="btn-theme" title="Theme">${themeIcon()}</button>`;
  if (state.view !== 'clone') {
    toolsHTML += `<button class="btn btn-secondary btn-sm" id="btn-clone-top">${ICONS.download}Clone</button>`;
  }

  ctx.innerHTML = ctxHTML;
  tools.innerHTML = toolsHTML;

  /* wire */
  const si = $('#search-input');
  if (si) si.addEventListener('input', () => { state.search = si.value; renderLibrary(); });
  const ss = $('#sort-select');
  if (ss) { ss.value = state.sort; ss.addEventListener('change', () => { state.sort = ss.value; renderLibrary(); }); }
  const tb = $('#topbar-back');
  if (tb) tb.addEventListener('click', () => showView('library'));
  const tbc = $('#topbar-back-collection');
  if (tbc) tbc.addEventListener('click', () => { state.activeCollectionId = null; showView('library'); });
  const brc = $('#btn-rename-collection');
  if (brc) brc.addEventListener('click', () => actRenameCollection(brc.dataset.id));
  const bdc = $('#btn-delete-collection');
  if (bdc) bdc.addEventListener('click', () => actDeleteCollection(bdc.dataset.id));
  $('#btn-theme').addEventListener('click', cycleTheme);
  const bc = $('#btn-clone-top');
  if (bc) bc.addEventListener('click', () => showView('clone'));
  updateThemeUI();
}

/* ============================ COLLECTIONS ============================ */
function collectionCoverModels(c) {
  return (c.model_ids || [])
    .map(id => state.models.find(m => m.id === id))
    .filter(Boolean)
    .slice(0, 4);
}

function collectionCollageHTML(c) {
  const covers = collectionCoverModels(c);
  const cells = [0, 1, 2, 3].map(i => {
    const m = covers[i];
    return m && m.cover_asset_url
      ? `<img src="${esc(m.cover_asset_url)}" alt="" loading="lazy" />`
      : `<span class="collage-empty"></span>`;
  }).join('');
  return `<div class="collection-collage">${cells}</div>`;
}

function renderSidebarCollections() {
  const wrap = $('#nav-collections');
  if (!wrap) return;
  if (!state.collections.length) {
    wrap.innerHTML = `<div class="nav-empty-hint">No collections yet</div>`;
    return;
  }
  wrap.innerHTML = state.collections.map(c => `
    <div class="collection-nav-item ${state.view === 'library' && state.activeCollectionId === c.id ? 'is-active' : ''}" data-collection="${c.id}" tabindex="0" role="button" aria-label="Open collection ${esc(c.name)}">
      ${collectionCollageHTML(c)}
      <span class="collection-name">${esc(c.name)}</span>
      <span class="nav-count">${(c.model_ids || []).length}</span>
      <span class="collection-item-actions">
        <button class="collection-icon-btn" data-action="rename" data-id="${c.id}" title="Rename collection">${ICONS.pencil}</button>
        <button class="collection-icon-btn" data-action="delete" data-id="${c.id}" title="Delete collection">${ICONS.trash}</button>
      </span>
    </div>`).join('');
}

function openCollection(id) {
  state.activeCollectionId = id;
  state.search = '';
  state.filter = 'all';
  showView('library');
}

async function refreshCollections() {
  if (!state.libraryConfigured) { state.collections = []; return; }
  try {
    state.collections = await Bridge.call('list_collections', {}) || [];
  } catch (err) {
    console.warn('Failed to load collections:', err);
    state.collections = [];
  }
}

function actCreateCollection() {
  const name = prompt('New collection name');
  if (name === null) return;
  const trimmed = name.trim();
  if (!trimmed) { toast('Collection name cannot be empty'); return; }
  Bridge.call('create_collection', { name: trimmed })
    .then(async () => {
      await refreshCollections();
      renderSidebarCollections();
      toast('Collection created', trimmed);
    })
    .catch(err => toast('Failed to create collection', err.message || err));
}

function actRenameCollection(id) {
  const c = state.collections.find(c => c.id === id);
  if (!c) return;
  const name = prompt('Rename collection', c.name);
  if (name === null) return;
  const trimmed = name.trim();
  if (!trimmed) { toast('Collection name cannot be empty'); return; }
  Bridge.call('rename_collection', { collectionId: id, name: trimmed })
    .then(async () => {
      await refreshCollections();
      renderSidebarCollections();
      if (state.view === 'library') renderTopbar();
      toast('Collection renamed', trimmed);
    })
    .catch(err => toast('Failed to rename collection', err.message || err));
}

function actDeleteCollection(id) {
  const c = state.collections.find(c => c.id === id);
  if (!c) return;
  if (!confirm(`Delete collection "${c.name}"?\n\nModels stay in your library — only the collection is removed.`)) return;
  Bridge.call('delete_collection', { collectionId: id })
    .then(async () => {
      if (state.activeCollectionId === id) state.activeCollectionId = null;
      await refreshCollections();
      renderSidebarCollections();
      showView('library');
      toast('Collection deleted', c.name);
    })
    .catch(err => toast('Failed to delete collection', err.message || err));
}

function renderCollectionPopover() {
  const pop = $('#collection-popover');
  if (!pop) return;
  const m = state.models.find(x => x.id === state.detailId);
  if (!m) { pop.innerHTML = ''; return; }
  const rows = state.collections.map(c => {
    const checked = (c.model_ids || []).includes(m.id);
    return `<label class="collection-pop-row">
      <input type="checkbox" data-collection="${c.id}" ${checked ? 'checked' : ''} />
      <span>${esc(c.name)}</span>
    </label>`;
  }).join('') || `<div class="collection-pop-empty">No collections yet</div>`;
  pop.innerHTML = `
    ${rows}
    <div class="collection-pop-new">
      <input type="text" id="collection-pop-input" placeholder="New collection…" autocomplete="off" spellcheck="false" />
      <button class="btn btn-primary btn-sm" id="collection-pop-create" type="button">Create</button>
    </div>`;
}

function toggleCollectionPopover(show) {
  const pop = $('#collection-popover');
  if (!pop) return;
  const next = show === undefined ? pop.classList.contains('hidden') : show;
  if (next) { renderCollectionPopover(); pop.classList.remove('hidden'); }
  else pop.classList.add('hidden');
}

/* ============================ LIBRARY ============================ */
function activeCollection() {
  return state.activeCollectionId ? state.collections.find(c => c.id === state.activeCollectionId) : null;
}

function filteredModels() {
  const q = state.search.trim().toLowerCase();
  const coll = activeCollection();
  let list = state.models.filter(m => {
    if (coll && !(coll.model_ids || []).includes(m.id)) return false;
    if (state.filter === 'plates' && !(m.files || []).some(f => extOf(f.n) === '3mf')) return false;
    if (state.filter === 'stl' && !(m.files || []).every(f => extOf(f.n) === 'stl')) return false;
    if (state.filter === 'recent' && !isRecent(m.added)) return false;
    if (q) {
      const hay = `${m.name} ${m.creator} ${(m.tags || []).join(' ')} ${m.source}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
  if (state.sort === 'name') list.sort((a, b) => a.name.localeCompare(b.name));
  else if (state.sort === 'size') list.sort((a, b) => modelSize(b) - modelSize(a));
  else list.sort((a, b) => (b.added || '').localeCompare(a.added || ''));
  return list;
}

function cardHTML(m) {
  const counts = extCounts(m);
  const extSummary = Object.keys(counts).map(e => `${counts[e]}× .${e}`).join(' · ');
  const badges = Object.keys(counts).map(e => `<span class="ext-badge ${extClass(e)}">.${e}</span>`).join('');
  const thumbHTML = m.cover_asset_url
    ? `<img class="thumb-img" src="${esc(m.cover_asset_url)}" alt="${esc(m.name)}" loading="lazy" />`
    : `<div class="thumb-inner">${GHOST}<span class="no-prev">no preview</span></div>`;
  return `
  <article class="card" data-id="${m.id}" tabindex="0" role="button" aria-label="Open ${esc(m.name)}">
    <div class="card-thumb">
      ${thumbHTML}
      <div class="ext-corner bottom">${badges}</div>
      <button class="btn btn-primary btn-sm card-quick" data-slice="${m.id}">Open in Slicer</button>
    </div>
    <div class="card-body">
      <h3 class="card-title">${esc(m.name)}</h3>
      <div class="card-creator">by ${esc(m.creator)}</div>
      <div class="card-meta">
        ${fmtMB(modelSize(m))}<span class="sep">·</span>${extSummary}<span class="sep">·</span>added ${fmtDate(m.added, true)}
      </div>
    </div>
  </article>`;
}

function renderLibrary() {
  const list = filteredModels();
  const grid = $('#grid'), empty = $('#empty'), emptyLib = $('#empty-library');

  if (!state.libraryConfigured) {
    grid.innerHTML = '';
    empty.classList.add('hidden');
    emptyLib.classList.remove('hidden');
  } else {
    grid.innerHTML = list.map(cardHTML).join('');
    empty.classList.toggle('hidden', list.length > 0);
    emptyLib.classList.add('hidden');
  }

  /* counts */
  const filtering = state.search.trim() !== '' || state.filter !== 'all';
  const lc = $('#lib-count');
  if (lc) lc.textContent = filtering ? `${list.length} of ${state.models.length}` : `${state.models.length} models`;
  $('#cnt-all').textContent = state.models.length;
  $('#cnt-plates').textContent = state.models.filter(m => (m.files || []).some(f => extOf(f.n) === '3mf')).length;
  $('#cnt-stl').textContent = state.models.filter(m => (m.files || []).every(f => extOf(f.n) === 'stl')).length;
  $('#cnt-recent').textContent = state.models.filter(m => isRecent(m.added)).length;
  $$('#chips .chip').forEach(c => c.classList.toggle('is-active', c.dataset.filter === state.filter));
  updateStats();
}

/* ============================ DETAIL ============================ */
const SHAPES = {
  grid: `<rect x="60" y="52" width="120" height="72" rx="7"/><circle cx="88" cy="76" r="8"/><circle cx="120" cy="76" r="8"/><circle cx="152" cy="76" r="8" stroke="var(--accent)"/><circle cx="88" cy="102" r="8"/><circle cx="120" cy="102" r="8"/><circle cx="152" cy="102" r="8"/>`,
  bracket: `<path d="M70 46h54v24H96v50H70z"/><circle cx="97" cy="58" r="6"/><circle cx="83" cy="106" r="6" stroke="var(--accent)"/><path d="M96 70h28" opacity=".45"/>`,
  hook: `<rect x="66" y="46" width="30" height="44" rx="6"/><path d="M96 62c34-10 52 8 44 30-6 17-28 22-38 10"/><circle cx="81" cy="104" r="5" stroke="var(--accent)"/>`,
  chain: `<rect x="62" y="58" width="116" height="54" rx="27"/><rect x="86" y="74" width="68" height="22" rx="11"/><circle cx="62" cy="85" r="6" stroke="var(--accent)"/><circle cx="178" cy="85" r="6" stroke="var(--accent)"/>`,
  ring: `<ellipse cx="120" cy="62" rx="48" ry="15"/><path d="M72 62v50a48 15 0 0 0 96 0V62"/><ellipse cx="120" cy="62" rx="19" ry="6" stroke="var(--accent)"/>`,
  hex: `<polygon points="96,48 75.2,60 75.2,84 96,96 116.8,84 116.8,60"/><polygon points="144,48 123.2,60 123.2,84 144,96 164.8,84 164.8,60"/><polygon points="120,90 99.2,102 99.2,126 120,138 140.8,126 140.8,102" fill="var(--accent)" fill-opacity=".13"/>`,
  hookset: `<rect x="62" y="44" width="116" height="84" rx="9"/><circle cx="86" cy="62" r="5"/><circle cx="120" cy="62" r="5"/><circle cx="154" cy="62" r="5"/><circle cx="86" cy="88" r="5"/><circle cx="154" cy="88" r="5"/><path d="M120 84v22a10 10 0 0 0 20 0v-6" stroke="var(--accent)"/>`,
  boxcase: `<rect x="62" y="56" width="116" height="64" rx="10"/><path d="M62 78h116"/><path d="M76 96v12M92 96v12M108 96v12" opacity=".5"/><rect x="150" y="64" width="18" height="8" rx="2" stroke="var(--accent)"/>`,
  slimcase: `<rect x="48" y="70" width="144" height="34" rx="9"/><rect x="58" y="78" width="96" height="18" rx="5" stroke-dasharray="3 4" opacity=".6"/><circle cx="172" cy="87" r="9" stroke="var(--accent)"/>`,
  cone: `<path d="M80 56h80l16 62H64z"/><ellipse cx="120" cy="56" rx="40" ry="9"/><path d="M96 68l-6 44M120 70v46M144 68l6 44" opacity=".45"/><path d="M74 118v7M120 118v7M166 118v7" stroke="var(--accent)"/>`,
  tray: `<rect x="58" y="58" width="124" height="60" rx="7"/><path d="M99 58v60M140 58v60" opacity=".6"/><path d="M58 88h124" opacity=".6"/>`,
  mount: `<path d="M70 96a50 50 0 0 1 100 0"/><path d="M70 96v18M170 96v18"/><rect x="104" y="52" width="32" height="20" rx="5" stroke="var(--accent)"/><path d="M84 114h72" opacity=".5"/>`,
  screw: `<rect x="96" y="40" width="48" height="18" rx="6"/><path d="M100 44v10M108 44v10M116 44v10M124 44v10M132 44v10M140 44v10" opacity=".55"/><rect x="106" y="58" width="28" height="66" rx="4"/><path d="M106 70l28 8M106 84l28 8M106 98l28 8M106 112l28 8" stroke="var(--accent)" opacity=".8"/>`,
  drum: `<ellipse cx="120" cy="56" rx="54" ry="16"/><path d="M66 56v60a54 16 0 0 0 108 0V56"/><circle cx="120" cy="56" r="8" stroke="var(--accent)"/><path d="M96 80v26M120 84v26M144 80v26" opacity=".45"/>`
};
const VIEW_LABELS = ['ISO', 'FRONT', 'DETAIL'];

function partSVG(m, variant) {
  const vb = variant === 2 ? '58 30 124 94' : '0 0 240 170';
  const tr = variant === 0 ? ' transform="translate(120 85) rotate(-9) skewX(-7) translate(-120 -85)"' : '';
  return `<svg viewBox="${vb}" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
    <g fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
      <line x1="36" y1="146" x2="204" y2="146" stroke-dasharray="2 5" opacity=".3"/>
      <g${tr}>${SHAPES[m.shape] || SHAPES.boxcase}</g>
    </g>
  </svg>`;
}

function renderDetail() {
  const m = state.models.find(x => x.id === state.detailId);
  if (!m) return;
  /* media */
  const hasImages = m.images && m.images.length > 0;
  const mediaHTML = hasImages
    ? `<img class="detail-cover-img" src="${esc(m.images[state.gallery].url)}" alt="${esc(m.name)}" />`
    : partSVG(m, state.gallery);
  $('#detail-media').innerHTML = mediaHTML;
  $('#media-cap-label').textContent = hasImages
    ? `Image ${state.gallery + 1} of ${m.images.length} · ${m.name}`
    : `${VIEW_LABELS[state.gallery]} view · ${m.name}`;
  /* strip */
  $('#detail-strip').innerHTML = hasImages
    ? m.images.map((img, i) =>
        `<button class="detail-strip-item ${i === state.gallery ? 'is-active' : ''}" data-i="${i}" title="Image ${i + 1}" aria-label="Show image ${i + 1}"><img class="strip-thumb-img" src="${esc(img.url)}" alt="image ${i + 1}" /></button>`
      ).join('')
    : [0, 1, 2].map(i =>
        `<button class="detail-strip-item ${i === state.gallery ? 'is-active' : ''}" data-i="${i}" title="${VIEW_LABELS[i]} view" aria-label="Show ${VIEW_LABELS[i]} view">${partSVG(m, i)}</button>`
      ).join('');
  /* files under gallery */
  $('#detail-files').innerHTML = `<h3>Files (${(m.files || []).length})</h3><div class="file-list">${
    (m.files || []).map(f => {
      const e = extOf(f.n);
      return `<div class="file-row">
        <div class="file-name"><span class="ext-badge ${extClass(e)}">.${e}</span><span>${esc(f.n)}</span></div>
        <span class="file-size">${fmtMB(f.mb)}</span>
        <span class="file-date">${fmtDate(f.modified || f.d, true)}</span>
        <button class="file-slice" data-file="${esc(f.n)}" title="Slice this file">${ICONS.layers}</button>
      </div>`;
    }).join('')
  }</div>`;
  /* side */
  $('#detail-title').textContent = m.name;
  $('#detail-creator').textContent = `by ${m.creator} · printables.com`;
  $('#detail-tags').innerHTML = (m.tags || []).map(t => `<button class="detail-tag" data-tag="${esc(t)}">#${esc(t)}</button>`).join('');
  /* description */
  const descEl = $('#detail-description');
  if (descEl) {
    const desc = m.description || '';
    if (desc.startsWith('Error:')) {
      descEl.innerHTML = `<p class="desc-error">${esc(desc)}</p>`;
    } else if (desc) {
      // Render markdown-like headers and paragraphs
      const html = desc.split('\n').map(line => {
        if (line.startsWith('## ')) return `<h4>${esc(line.slice(3))}</h4>`;
        if (line.trim() === '') return '';
        // Convert [text](url) links
        const withLinks = line.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" class="desc-link" data-url="$2">$1</a>');
        return `<p>${withLinks}</p>`;
      }).filter(Boolean).join('');
      descEl.innerHTML = html;
    } else {
      descEl.innerHTML = '<p class="desc-empty">No description available.</p>';
    }
  }
  $('#detail-meta').innerHTML = `
    <dt>Total size</dt><dd class="mono">${fmtMB(modelSize(m))} · ${(m.files || []).length} file${(m.files || []).length > 1 ? 's' : ''}</dd>
    <dt>Added</dt><dd>${fmtDate(m.added)}</dd>
    <dt>Source</dt><dd><span class="src-link" id="src-link">${esc((m.source || '').replace('https://www.', ''))}</span></dd>
    <dt>Path</dt><dd class="mono">${esc(localPath(m))}</dd>`;
}

function openDetail(id) {
  state.detailId = id;
  state.gallery = 0;
  toggleCollectionPopover(false);
  renderDetail();
  showView('detail');
}

function setGallery(i) {
  const m = state.models.find(x => x.id === state.detailId);
  if (!m) return;
  const hasImages = m.images && m.images.length > 0;
  if (hasImages) {
    state.gallery = ((i % m.images.length) + m.images.length) % m.images.length;
    $('#detail-media').innerHTML = `<img class="detail-cover-img" src="${esc(m.images[state.gallery].url)}" alt="${esc(m.name)}" />`;
    $('#media-cap-label').textContent = `Image ${state.gallery + 1} of ${m.images.length} · ${m.name}`;
  } else {
    state.gallery = ((i % 3) + 3) % 3;
    $('#detail-media').innerHTML = partSVG(m, state.gallery);
    $('#media-cap-label').textContent = `${VIEW_LABELS[state.gallery]} view · ${m.name}`;
  }
  $$('#detail-strip .detail-strip-item').forEach(b => b.classList.toggle('is-active', Number(b.dataset.i) === state.gallery));
  if (!$('#lightbox').classList.contains('hidden')) renderLightbox();
}

/* ============================ LIGHTBOX ============================ */
function renderLightbox() {
  const m = state.models.find(x => x.id === state.detailId);
  if (!m) return;
  const hasImages = m.images && m.images.length > 0;
  const figHTML = hasImages
    ? `<img class="detail-cover-img" src="${esc(m.images[state.gallery].url)}" alt="${esc(m.name)}" style="width:100%;height:auto;max-height:70vh;object-fit:contain" />
       <div class="media-cap"><span>Image ${state.gallery + 1} · ${esc(m.name)}</span><span>${state.gallery + 1} / ${m.images.length} · downloaded preview</span></div>`
    : `${partSVG(m, state.gallery)}
       <div class="media-cap"><span>${VIEW_LABELS[state.gallery]} view · ${esc(m.name)}</span><span>${state.gallery + 1} / 3 · generated preview</span></div>`;
  $('#lb-fig').innerHTML = figHTML;
}
function openLightbox() { renderLightbox(); $('#lightbox').classList.remove('hidden'); }
function closeLightbox() { $('#lightbox').classList.add('hidden'); }

/* ============================ CLONE ============================ */
const cloneTimers = [];
const sleep = ms => new Promise(r => cloneTimers.push(setTimeout(r, ms)));

function queueRow(name) {
  const li = document.createElement('li');
  li.dataset.name = name;
  li.innerHTML = `<span class="fname">${esc(name)}</span><span class="fstatus">queued</span>`;
  $('#clone-queue').appendChild(li);
}

function queueSet(name, text, done) {
  const li = $(`#clone-queue li[data-name="${CSS.escape(name)}"]`);
  if (!li) return;
  const st = li.querySelector('.fstatus');
  st.textContent = done ? 'done ✓' : text;
  st.classList.toggle('done', !!done);
}

function setStep(t) { $('#clone-step').textContent = t; }

async function startClone(url) {
  const form = $('#clone-form');
  $('#clone-submit').disabled = true;
  $('#clone-url').disabled = true;
  $('#clone-error').classList.add('hidden');
  $('#clone-progress').classList.remove('hidden');
  $('#clone-success').classList.add('hidden');
  $('#clone-queue').innerHTML = '';
  $('#clone-pct').textContent = '';

  const slugRaw = (url.match(/\/model\/\d+-([^/?#]+)/) || [])[1]
    || url.replace(/^https?:\/\//, '').split(/[/?#]/).filter(Boolean).pop()
    || 'cloned-model';
  const slug = slugRaw.toLowerCase().replace(/[^\w-]+/g, '-').replace(/-{2,}/g, '-').replace(/^-|-$/g, '').slice(0, 48) || 'cloned-model';

  setStep('Fetching model metadata…');
  await sleep(400);

  try {
    const result = await Bridge.call('clone_model', { url });
    const jobId = result.job_id;

    /* Listen for progress events */
    const unlistenProgress = await Bridge.listen(`clone://${jobId}/progress/phase`, (event) => {
      const p = event.payload;
      if (p.phase === 'metadata') setStep('Fetching model metadata…');
      else if (p.phase === 'files') setStep(`Downloading files (${p.file_index || 0} of ${p.file_total || '?'})…`);
      else if (p.phase === 'images') setStep('Downloading images…');
      else if (p.phase === 'finalize') setStep('Finalizing…');
      if (p.percent !== undefined) $('#clone-pct').textContent = Math.round(p.percent) + '%';
    });

    const unlistenDone = await Bridge.listen(`clone://${jobId}/done`, async () => {
      unlistenProgress();
      unlistenDone();
      unlistenError();
      await refreshLibrary();
      $('#clone-progress').classList.add('hidden');
      $('#success-title').textContent = `${titleCase(slug.replace(/[-_]+/g, ' '))} cloned`;
      $('#success-path').textContent = `${state.prefs.library_folder || '~/Printables Library'}/${slug}/`;
      $('#clone-success').classList.remove('hidden');
      toast('Model added to library', 'clone complete');
    });

    const unlistenError = await Bridge.listen(`clone://${jobId}/error`, (event) => {
      unlistenProgress();
      unlistenDone();
      unlistenError();
      const err = event.payload;
      $('#clone-progress').classList.add('hidden');
      $('#clone-error').textContent = `Clone failed: ${err.message || 'unknown error'}`;
      $('#clone-error').classList.remove('hidden');
      $('#clone-submit').disabled = false;
      $('#clone-url').disabled = false;
      toast('Clone failed', err.message || 'unknown error');
    });

  } catch (err) {
    $('#clone-progress').classList.add('hidden');
    $('#clone-error').textContent = `Clone failed: ${err.message || err}`;
    $('#clone-error').classList.remove('hidden');
    $('#clone-submit').disabled = false;
    $('#clone-url').disabled = false;
    toast('Clone failed', err.message || err);
  }
}

function resetCloneForm() {
  $('#clone-form').classList.remove('hidden');
  $('#clone-url').disabled = false;
  $('#clone-url').value = '';
  $('#clone-submit').disabled = false;
  $('#clone-error').classList.add('hidden');
  $('#clone-note').classList.add('hidden');
  $('#clone-progress').classList.add('hidden');
  $('#clone-success').classList.add('hidden');
}

function renderRecent() {
  const recent = [...state.models].sort((a, b) => (b.added || '').localeCompare(a.added || '')).slice(0, 3);
  $('#recent-list').innerHTML = recent.map(m => `
    <li><button class="recent-item" data-id="${m.id}">
      ${ICONS.download}
      <span class="rname">${esc(m.name)}</span>
      <span class="rsrc">${esc((m.source || '').replace('https://www.', ''))}</span>
      <span class="rdate">${fmtDate(m.added, true)}</span>
    </button></li>`).join('');
}

/* ============================ STATS / STORAGE ============================ */
function updateStats() {
  const totalMB = libTotalMB();
  const usedGB = (totalMB / 1024).toFixed(1);

  $('#nav-count').textContent = state.models.length;
  $('#storage-nums').textContent = `${usedGB} GB`;
  $('#storage-meta').textContent = `${state.models.length} models · ${fileCount()} files indexed`;
  $('#settings-storage-nums').textContent = `${usedGB} GB used`;
}

/* ============================ VIEWS ============================ */
function showView(v) {
  state.view = v;
  $$('.view').forEach(sec => sec.classList.toggle('is-active', sec.id === 'view-' + v));
  $$('.nav-item[data-view]').forEach(b => b.classList.toggle('is-active', !state.activeCollectionId && b.dataset.view === (v === 'detail' ? 'library' : v)));
  renderSidebarCollections();
  renderTopbar();
  if (v === 'library') renderLibrary();
  window.scrollTo(0, 0);
}

/* ============================ ACTIONS ============================ */
function slicerName() { return SLICERS[state.prefs.slicer_key]?.[0] || 'PrusaSlicer'; }

function actOpenInSlicer(m) {
  const files = (m.files || []).map(f => localPath(m) + f.n);
  Bridge.call('open_in_slicer', { modelId: m.id, slicer: slicerName(), files })
    .then(() => toast(`Launching ${slicerName()}…`, 'invoke open_in_slicer'))
    .catch(err => toast('Failed to launch slicer', err.message || err));
}

function actExport(m) {
  Bridge.call('export_files', { modelId: m.id })
    .then(() => toast('Export complete', 'files copied to destination'))
    .catch(err => toast('Export failed', err.message || err));
}

function actFolder(m) {
  const path = localPath(m);
  Bridge.call('open_folder', { path })
    .then(() => toast('Opening folder…', 'invoke open_folder'))
    .catch(err => toast('Failed to open folder', err.message || err));
}

function actDelete(m) {
  if (!confirm(`Delete "${m.name}" from your library?\n\nThis will permanently remove all files.`)) return;
  Bridge.call('delete_model', { modelId: m.id })
    .then(async () => {
      toast('Model deleted', m.name);
      // Refresh library and go back to library view
      await refreshLibrary();
      await refreshCollections();
      renderSidebarCollections();
      showView('library');
    })
    .catch(err => toast('Failed to delete model', err.message || err));
}

function actSliceFile(name) {
  const m = state.models.find(x => x.id === state.detailId);
  const file = localPath(m) + name;
  Bridge.call('slice_file', { file })
    .then(() => toast(`Slicing ${name}…`, 'invoke slice_file'))
    .catch(err => toast('Failed to slice', err.message || err));
}

function actOpenExternal(url) {
  Bridge.call('open_external', { url })
    .then(() => toast('Opening URL…', 'invoke open_external'))
    .catch(err => toast('Failed to open URL', err.message || err));
}

/* ============================ DATA LOADING ============================ */
async function loadPrefs() {
  try {
    const prefs = await Bridge.call('get_prefs', {});
    state.prefs = { ...state.prefs, ...prefs };
    /* Sync localStorage with backend prefs */
    if (prefs.theme) {
      state.prefs.theme = prefs.theme;
      try { localStorage.setItem(THEME_KEY, prefs.theme); } catch(e) {}
    }
    if (prefs.slicer_key) {
      state.prefs.slicer_key = prefs.slicer_key;
      try { localStorage.setItem(SLICER_KEY, prefs.slicer_key); } catch(e) {}
    }
    state.libraryConfigured = !!prefs.library_folder;
  } catch (err) {
    console.warn('Failed to load prefs:', err);
  }
}

async function refreshLibrary() {
  if (!state.libraryConfigured) {
    state.models = [];
    state.totals = { models: 0, files: 0, bytes_used: 0, bytes_capacity: DISK_GB * 1024 * 1024 * 1024 };
    return;
  }
  try {
    const idx = await Bridge.call('list_models', {});
    state.models = idx.models || [];
    state.totals = idx.totals || state.totals;
  } catch (err) {
    console.warn('Failed to load library:', err);
    state.models = [];
  }
}

/* ============================ EVENT WIRING ============================ */
/* nav */
$$('.nav-item[data-view]').forEach(b => b.addEventListener('click', () => {
  state.activeCollectionId = null;
  showView(b.dataset.view);
}));

/* collections sidebar */
$('#btn-new-collection').addEventListener('click', actCreateCollection);
$('#nav-collections').addEventListener('click', e => {
  const actionBtn = e.target.closest('.collection-icon-btn');
  if (actionBtn) {
    e.stopPropagation();
    const id = actionBtn.dataset.id;
    if (actionBtn.dataset.action === 'rename') actRenameCollection(id);
    else if (actionBtn.dataset.action === 'delete') actDeleteCollection(id);
    return;
  }
  const item = e.target.closest('.collection-nav-item');
  if (item) openCollection(item.dataset.collection);
});

/* add to collection (detail page) */
$('#act-add-collection').addEventListener('click', e => {
  e.stopPropagation();
  toggleCollectionPopover();
});
document.addEventListener('click', e => {
  const pop = $('#collection-popover');
  if (!pop || pop.classList.contains('hidden')) return;
  if (!e.target.closest('.collection-btn-wrap')) toggleCollectionPopover(false);
});
$('#collection-popover').addEventListener('change', e => {
  const cb = e.target.closest('input[type="checkbox"][data-collection]');
  if (!cb) return;
  const collectionId = cb.dataset.collection;
  const modelId = state.detailId;
  const command = cb.checked ? 'add_model_to_collection' : 'remove_model_from_collection';
  Bridge.call(command, { collectionId, modelId })
    .then(async () => {
      await refreshCollections();
      renderSidebarCollections();
      toast(cb.checked ? 'Added to collection' : 'Removed from collection');
    })
    .catch(err => {
      cb.checked = !cb.checked;
      toast('Failed to update collection', err.message || err);
    });
});
$('#collection-popover').addEventListener('click', e => {
  if (e.target.id !== 'collection-pop-create') return;
  e.preventDefault();
  const input = $('#collection-pop-input');
  const name = (input.value || '').trim();
  if (!name) return;
  Bridge.call('create_collection', { name })
    .then(c => Bridge.call('add_model_to_collection', { collectionId: c.id, modelId: state.detailId }))
    .then(async () => {
      await refreshCollections();
      renderSidebarCollections();
      renderCollectionPopover();
      toast('Added to new collection', name);
    })
    .catch(err => toast('Failed to create collection', err.message || err));
});

/* library grid — card open + quick slice */
$('#grid').addEventListener('click', e => {
  const quick = e.target.closest('.card-quick');
  if (quick) {
    e.stopPropagation();
    actOpenInSlicer(state.models.find(m => m.id === quick.dataset.slice));
    return;
  }
  const card = e.target.closest('.card');
  if (card) openDetail(card.dataset.id);
});
$('#grid').addEventListener('keydown', e => {
  if (e.key !== 'Enter' && e.key !== ' ') return;
  const card = e.target.closest('.card');
  if (card && e.target === card) { e.preventDefault(); openDetail(card.dataset.id); }
});

/* filter chips */
$('#chips').addEventListener('click', e => {
  const chip = e.target.closest('.chip');
  if (!chip) return;
  state.filter = chip.dataset.filter;
  renderLibrary();
});

/* empty state reset */
$('#empty-clear').addEventListener('click', () => {
  state.search = '';
  state.filter = 'all';
  renderTopbar();
  renderLibrary();
});

$('#empty-go-settings').addEventListener('click', () => showView('settings'));
$('#empty-go-clone').addEventListener('click', () => showView('clone'));

/* detail — strip, media, tags, actions, files */
$('#detail-strip').addEventListener('click', e => {
  const item = e.target.closest('.detail-strip-item');
  if (item) setGallery(Number(item.dataset.i));
});
$('#detail-media').addEventListener('click', openLightbox);
$('#detail-back').addEventListener('click', () => showView('library'));
$('#detail-tags').addEventListener('click', e => {
  const tag = e.target.closest('.detail-tag');
  if (!tag) return;
  state.search = tag.dataset.tag;
  state.filter = 'all';
  showView('library');
});
$('#detail-files').addEventListener('click', e => {
  const b = e.target.closest('.file-slice');
  if (b) actSliceFile(b.dataset.file);
});
$('#act-slicer').addEventListener('click', () => actOpenInSlicer(state.models.find(m => m.id === state.detailId)));
$('#act-export').addEventListener('click', () => actExport(state.models.find(m => m.id === state.detailId)));
$('#act-folder').addEventListener('click', () => actFolder(state.models.find(m => m.id === state.detailId)));
$('#act-delete').addEventListener('click', () => actDelete(state.models.find(m => m.id === state.detailId)));
$('#detail-meta').addEventListener('click', e => {
  if (e.target.closest('#src-link')) {
    const m = state.models.find(x => x.id === state.detailId);
    actOpenExternal(m.source);
  }
});
$('#detail-description').addEventListener('click', e => {
  const link = e.target.closest('.desc-link');
  if (link) {
    e.preventDefault();
    actOpenExternal(link.dataset.url);
  }
});

/* lightbox */
$('#lb-close').addEventListener('click', closeLightbox);
$('#lb-prev').addEventListener('click', () => {
  const m = state.models.find(x => x.id === state.detailId);
  if (m) setGallery(state.gallery - 1);
});
$('#lb-next').addEventListener('click', () => {
  const m = state.models.find(x => x.id === state.detailId);
  if (m) setGallery(state.gallery + 1);
});
$('#lightbox').addEventListener('click', e => { if (e.target.id === 'lightbox') closeLightbox(); });

/* clone */
$('#clone-form').addEventListener('submit', e => {
  e.preventDefault();
  const input = $('#clone-url');
  const url = input.value.trim();
  const err = $('#clone-error'), note = $('#clone-note');
  err.classList.add('hidden'); note.classList.add('hidden');
  if (!url) { err.textContent = 'Paste a model URL to clone.'; err.classList.remove('hidden'); input.focus(); return; }
  try { new URL(url); } catch(_) { err.textContent = 'That doesn\'t look like a valid URL.'; err.classList.remove('hidden'); input.focus(); return; }
  if (!/printables\.com\/model\/\d+/.test(url)) note.classList.remove('hidden');
  startClone(url);
});
$('#clone-url').addEventListener('input', () => { $('#clone-error').classList.add('hidden'); $('#clone-note').classList.add('hidden'); });
$('#success-view').addEventListener('click', () => {
  const recent = [...state.models].sort((a, b) => (b.added || '').localeCompare(a.added || ''))[0];
  if (recent) openDetail(recent.id);
});
$('#success-again').addEventListener('click', resetCloneForm);
$('#recent-list').addEventListener('click', e => {
  const item = e.target.closest('.recent-item');
  if (item) openDetail(item.dataset.id);
});

/* settings */
$('#theme-seg').addEventListener('click', e => {
  const b = e.target.closest('button[data-pref]');
  if (b) applyTheme(b.dataset.pref);
});

function setSlicerPath() {
  $('#slicer-path').value = SLICERS[state.prefs.slicer_key]?.[1] || '';
}

$('#slicer-select').value = state.prefs.slicer_key || 'prusa';
setSlicerPath();
$('#slicer-select').addEventListener('change', e => {
  state.prefs.slicer_key = e.target.value;
  try { localStorage.setItem(SLICER_KEY, state.prefs.slicer_key); } catch(_) {}
  setSlicerPath();
  Bridge.call('set_prefs', { slicer_key: state.prefs.slicer_key }).catch(() => {});
  toast(`Default slicer set to ${slicerName()}`, 'synced to backend');
});

$('#slicer-browse').addEventListener('click', async () => {
  try {
    const path = await Bridge.call('dialog_open', { kind: 'file' });
    if (path) {
      $('#slicer-path').value = path;
      Bridge.call('set_prefs', { slicer_executable: path }).catch(() => {});
      toast('Slicer executable set', path);
    }
  } catch (err) {
    toast('Failed to open file picker', err.message || err);
  }
});

$('#folder-browse').addEventListener('click', async () => {
  try {
    const path = await Bridge.call('dialog_open', { kind: 'folder' });
    if (path) {
      $('#library-path').value = path;
      Bridge.call('set_prefs', { library_folder: path }).then((prefs) => {
        state.libraryConfigured = true;
        state.prefs = { ...state.prefs, ...prefs };
        refreshLibrary().then(async () => {
          await refreshCollections();
          renderSidebarCollections();
          renderLibrary();
          toast('Library folder set', prefs.library_folder || path);
        });
      }).catch(err => toast('Failed to set library folder', err.message || err));
    }
  } catch (err) {
    toast('Failed to open folder picker', err.message || err);
  }
});

$('#maint-rescan').addEventListener('click', () => {
  Bridge.call('rescan_library', {})
    .then(async (idx) => {
      state.models = idx.models || [];
      state.totals = idx.totals || state.totals;
      await refreshCollections();
      renderSidebarCollections();
      renderLibrary();
      toast('Library rescanned', `${state.models.length} models indexed`);
    })
    .catch(err => toast('Failed to rescan library', err.message || err));
});

/* GitHub link */
$('#github-link').addEventListener('click', e => {
  e.preventDefault();
  actOpenExternal('https://github.com/JacRob32/Printables-Offline');
});

/* keyboard */
document.addEventListener('keydown', e => {
  const ae = document.activeElement;
  const typing = ae && /^(INPUT|SELECT|TEXTAREA)$/.test(ae.tagName);
  if (e.key === '/' && !typing) {
    e.preventDefault();
    if (state.view !== 'library') showView('library');
    const s = $('#search-input');
    if (s) s.focus();
  } else if (e.key === 'Escape') {
    if (!$('#lightbox').classList.contains('hidden')) closeLightbox();
    else if (typing) ae.blur();
    else if (state.view === 'detail') showView('library');
  } else if ((e.key === 'ArrowRight' || e.key === 'ArrowLeft') && !typing && state.view === 'detail') {
    const m = state.models.find(x => x.id === state.detailId);
    if (m && !m.cover_asset_url) setGallery(state.gallery + (e.key === 'ArrowRight' ? 1 : -1));
  }
});

/* ============================ INIT ============================ */
async function init() {
  await loadPrefs();
  applyTheme(state.prefs.theme, true);
  await refreshLibrary();
  await refreshCollections();
  renderSidebarCollections();
  renderTopbar();
  renderLibrary();
  renderRecent();
  updateStats();
  console.info('%cPrintables Offline%c — Tauri IPC active.', 'font-weight:700;color:#FF5701', '');
}

init();
