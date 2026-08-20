/* theme.js - synchronous theme bootstrap. Loaded in <head> (no defer) so the
   correct color-scheme is applied before first paint (no flash of the wrong
   theme). Persists under localStorage 'rc.theme' (light | dark | system).
   Dark-first: first-time visitors get dark mode; 'system' still follows the
   OS. Exposes window.AppTheme for the UI (header toggle, settings, palette). */

(function () {
  var KEY = 'rc.theme';
  var STORAGE = (function () {
    try { return window.localStorage; } catch (e) { return null; }
  })();

  function read() {
    try {
      var raw = STORAGE.getItem(KEY);
      if (raw === 'light' || raw === 'dark') return raw;
    } catch (e) { /* ignore */ }
    return 'dark';
  }

  function resolve(pref) {
    if (pref === 'light' || pref === 'dark') return pref;
    var dark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    return dark ? 'dark' : 'light';
  }

  function apply(pref) {
    var mode = resolve(pref);
    var root = document.documentElement;
    root.setAttribute('data-theme', mode);
    root.style.colorScheme = mode;
    return mode;
  }

  var listeners = [];
  var current = read();

  window.AppTheme = {
    init: function () { apply(current); },
    current: function () { return current; },
    mode: function () { return apply(current); },
    set: function (pref) {
      current = pref;
      try { STORAGE.setItem(KEY, pref); } catch (e) { /* ignore */ }
      var mode = apply(pref);
      listeners.forEach(function (fn) { fn(mode, pref); });
      return mode;
    },
    toggle: function () {
      var next = apply(current) === 'dark' ? 'light' : 'dark';
      return window.AppTheme.set(next);
    },
    onChange: function (fn) { listeners.push(fn); },
    icon: function () { return resolve(current) === 'dark' ? 'sun' : 'moon'; },
  };

  if (window.matchMedia) {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function () {
      if (current === 'system') apply('system');
    });
  }

  window.AppTheme.init();
})();