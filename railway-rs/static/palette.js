/* palette.js - Command palette (Cmd+K / Ctrl+K). Smart query parsing,
   recent lookups, quick actions, and search results. */

(() => {
  let isOpen = false;
  let currentItems = [];
  let highlight = -1;
  let token = 0;
  let backdrop = null;

  /* ---------- Smart query parser ---------- */

  function parseQuery(q) {
    const clean = q.trim().toUpperCase();
    if (!clean) return null;

    // PNR: exactly 10 digits
    if (/^\d{10}$/.test(clean)) return { type: 'pnr', pnr: clean };

    // Plan: two station codes separated by space, >, or arrow
    const planMatch = clean.match(/^([A-Z0-9]{2,4})\s*[>\u2192]\s*([A-Z0-9]{2,4})$/);
    if (planMatch) return { type: 'plan', src: planMatch[1], dst: planMatch[2] };

    // Plan: two station codes separated by space (both 2-4 chars)
    const planSpace = clean.match(/^([A-Z0-9]{2,4})\s+([A-Z0-9]{2,4})$/);
    if (planSpace && planSpace[1] !== planSpace[2]) return { type: 'plan', src: planSpace[1], dst: planSpace[2] };

    // Train + view: "12559 delay"
    const trainView = clean.match(/^(\d{1,8})\s+(SCHEDULE|MAP|DELAY|EXCEPTIONS|JOURNEY|SPOT)$/);
    if (trainView) return { type: 'train', train: trainView[1], view: trainView[2].toLowerCase() };

    // Train number only
    if (/^\d{1,8}$/.test(clean)) return { type: 'train', train: clean };

    // Station + view: "NDLS TIMETABLE"
    const stationView = clean.match(/^([A-Z0-9]{2,4})\s+(TIMETABLE|TT|HERITAGE|PARCEL|LIVE)$/);
    if (stationView) {
      const view = stationView[2] === 'TIMETABLE' || stationView[2] === 'TT' ? 'tt' : stationView[2].toLowerCase();
      return { type: 'station', station: stationView[1], view: view };
    }

    // Station code only (2-4 alphanumeric)
    if (/^[A-Z0-9]{2,4}$/.test(clean)) return { type: 'station', station: clean };

    // System commands
    if (/^(OBS|OBSERVABILITY)$/i.test(clean)) return { type: 'system', view: 'observability' };
    if (/^DEBUG$/i.test(clean)) return { type: 'system', view: 'debug' };
    if (/^SETTINGS?$/i.test(clean)) return { type: 'system', view: 'settings' };
    if (/^(THEME|DARK|LIGHT)$/i.test(clean)) return { type: 'theme', mode: clean.toLowerCase() };

    // Fallback: search
    return { type: 'search', query: q.trim() };
  }

  function parsedToNav(parsed) {
    if (!parsed) return null;
    switch (parsed.type) {
      case 'pnr':
        return Routes.href({ section: 'train', params: { _pnr: parsed.pnr } });
      case 'train':
        return Routes.href({ section: 'train', view: parsed.view, params: { train: parsed.train } });
      case 'station':
        return Routes.href({ section: 'station', view: parsed.view, params: { station: parsed.station } });
      case 'plan':
        return Routes.href({ section: 'plan', params: { src: parsed.src, dst: parsed.dst } });
      case 'system':
        return Routes.href({ section: 'system', view: parsed.view });
      default:
        return null;
    }
  }

  function parsedLabel(parsed) {
    if (!parsed) return '';
    switch (parsed.type) {
      case 'pnr': return 'Check PNR ' + parsed.pnr;
      case 'train': return 'Train ' + parsed.train + (parsed.view ? ' \u2192 ' + parsed.view : '');
      case 'station': return 'Station ' + parsed.station + (parsed.view ? ' \u2192 ' + parsed.view : '');
      case 'plan': return parsed.src + ' \u2192 ' + parsed.dst;
      case 'system': return parsed.view.charAt(0).toUpperCase() + parsed.view.slice(1);
      default: return parsed.query || '';
    }
  }

  function parsedIcon(parsed) {
    if (!parsed) return 'search';
    switch (parsed.type) {
      case 'pnr': return 'ticket';
      case 'train': return 'train';
      case 'station': return 'station';
      case 'plan': return 'map';
      case 'system': return 'settings';
      case 'theme': return 'sun';
      default: return 'search';
    }
  }

  function iconItem(cls, name) {
    const box = window.UI.el('span', { class: cls });
    box.append(window.UI.icon(name));
    return box;
  }

  /* ---------- Palette UI ---------- */

  function getRecent() {
    try {
      const raw = localStorage.getItem('rc.recent');
      const arr = raw ? JSON.parse(raw) : [];
      return Array.isArray(arr) ? arr : [];
    } catch { return []; }
  }

  function buildPalette() {
    const UI = window.UI;
    backdrop = UI.el('div', { class: 'palette-backdrop' });
    const panel = UI.el('div', { class: 'palette' });
    const input = UI.el('input', {
      class: 'palette-input',
      placeholder: 'Search trains, stations, or type a command...',
      autocomplete: 'off',
      spellcheck: 'false',
    });
    const body = UI.el('div', { class: 'palette-body' });

    backdrop.addEventListener('click', (e) => { if (e.target === backdrop) close(); });
    input.addEventListener('keydown', (e) => onKeydown(e, body));
    input.addEventListener('input', () => onInput(input.value, body));

    panel.append(input, body);
    backdrop.append(panel);
    document.body.append(backdrop);

    // Render initial state
    renderInitial(body);
    input.focus();
  }

  function renderInitial(body) {
    const UI = window.UI;
    body.replaceChildren();
    currentItems = [];
    highlight = -1;

    // Recent
    const recent = getRecent().slice(0, 5);
    if (recent.length) {
      const sec = UI.el('div', { class: 'palette-section' });
      sec.append(UI.el('div', { class: 'palette-section-label', text: 'Recent' }));
      recent.forEach((r) => {
        const entityType = r.hash.includes('/train/') ? 'train'
          : r.hash.includes('/station/') ? 'station'
          : r.hash.includes('/plan/') ? 'plan'
          : r.hash.includes('/pnr/') ? 'pnr' : '';
        const icons = { train: 'train', station: 'station', plan: 'map', pnr: 'ticket' };
        const item = UI.el('div', {
          class: 'palette-item',
          'data-hash': r.hash,
          onclick: () => { navigate(r.hash); close(); },
        });
        item.append(
          iconItem('pi-icon', icons[entityType] || 'clock'),
          UI.el('span', { class: 'pi-text', text: r.label }),
          UI.el('span', { class: 'pi-hint', text: r.hash }),
        );
        sec.append(item);
        currentItems.push(item);
      });
      body.append(sec);
    }

    // Quick actions
    const actions = [
      { icon: 'home', label: 'Go to Home', hash: '#/' },
      { icon: 'pulse', label: 'Open Observability', hash: '#/system/observability' },
      { icon: 'settings', label: 'System Settings', hash: '#/system/settings' },
      { icon: 'log', label: 'Debug Log', hash: '#/system/debug' },
    ];
    const sec2 = UI.el('div', { class: 'palette-section' });
    sec2.append(UI.el('div', { class: 'palette-section-label', text: 'Actions' }));
    actions.forEach((a) => {
      const item = UI.el('div', {
        class: 'palette-item',
        onclick: () => { navigate(a.hash); close(); },
      });
      item.append(
        iconItem('pi-icon', a.icon),
        UI.el('span', { class: 'pi-text', text: a.label }),
      );
      sec2.append(item);
      currentItems.push(item);
    });
    body.append(sec2);
  }

  async function onInput(query, body) {
    const UI = window.UI;
    const my = ++token;

    if (!query.trim()) {
      renderInitial(body);
      return;
    }

    // Check smart parse first
    const parsed = parseQuery(query);
    if (parsed && parsed.type !== 'search') {
      if (parsed.type === 'theme') {
        if (window.AppTheme) window.AppTheme.set(parsed.mode);
        close();
        return;
      }
      body.replaceChildren();
      currentItems = [];
      highlight = -1;
      const sec = UI.el('div', { class: 'palette-section' });
      sec.append(UI.el('div', { class: 'palette-section-label', text: 'Result' }));
      const hash = parsedToNav(parsed);
      const item = UI.el('div', {
        class: 'palette-item hl',
        onclick: () => { navigate(hash); close(); },
      });
      item.append(
        iconItem('pi-icon', parsedIcon(parsed)),
        UI.el('span', { class: 'pi-text', text: parsedLabel(parsed) }),
        UI.el('span', { class: 'pi-hint', text: hash }),
      );
      sec.append(item);
      body.append(sec);
      currentItems = [item];
      highlight = 0;
      return;
    }

    // Fallback: search via API
    try {
      const suggestions = await window.Api.suggest(query);
      if (my !== token) return;
      body.replaceChildren();
      currentItems = [];
      highlight = -1;

      if (!Array.isArray(suggestions) || !suggestions.length) {
        body.append(UI.el('div', { class: 'palette-empty', text: 'No results found' }));
        return;
      }

      const sec = UI.el('div', { class: 'palette-section' });
      sec.append(UI.el('div', { class: 'palette-section-label', text: 'Results' }));

      suggestions.forEach((s) => {
        const isTrain = s.type === 'train';
        const hash = isTrain
          ? Routes.href({ section: 'train', params: { train: s.number } })
          : Routes.href({ section: 'station', params: { station: s.code } });
        const item = UI.el('div', {
          class: 'palette-item',
          onclick: () => { navigate(hash); close(); },
        });
        item.append(
          iconItem('pi-icon', isTrain ? 'train' : 'station'),
          UI.el('span', { class: 'pi-text', text: s.name }),
          UI.el('span', { class: 'pi-hint', text: isTrain ? s.number : s.code }),
        );
        sec.append(item);
        currentItems.push(item);
      });
      body.append(sec);
    } catch (err) {
      body.replaceChildren(UI.el('div', { class: 'palette-empty', text: 'Search failed' }));
    }
  }

  function onKeydown(e, body) {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (currentItems.length) {
        highlight = (highlight + 1) % currentItems.length;
        updateHighlight();
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (currentItems.length) {
        highlight = (highlight - 1 + currentItems.length) % currentItems.length;
        updateHighlight();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (highlight >= 0 && currentItems[highlight]) {
        currentItems[highlight].click();
      }
    }
  }

  function updateHighlight() {
    currentItems.forEach((item, i) => item.classList.toggle('hl', i === highlight));
    if (highlight >= 0 && currentItems[highlight]) {
      currentItems[highlight].scrollIntoView({ block: 'nearest' });
    }
  }

  /* ---------- Open / Close ---------- */

  function open() {
    if (isOpen) return;
    isOpen = true;
    buildPalette();
  }

  function close() {
    if (!isOpen || !backdrop) return;
    isOpen = false;
    backdrop.remove();
    backdrop = null;
    currentItems = [];
    highlight = -1;
  }

  function navigate(hash) {
    if (location.hash === hash) {
      // Force re-render
      const route = Routes.parse(hash);
      if (route) {
        const root = document.getElementById('tab-root');
        if (root) {
          const section = window.Sections[route.section];
          if (section) section.mount(root, window._appCtx ? window._appCtx() : {}, route);
        }
      }
    } else {
      location.hash = hash;
    }
  }

  /* ---------- Global keyboard listener ---------- */

  document.addEventListener('keydown', (e) => {
    // Cmd+K or Ctrl+K
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      if (isOpen) close();
      else open();
    }
  });

  /* ---------- Expose open() for search bar click ---------- */
  window.Palette = { open, close };
})();
