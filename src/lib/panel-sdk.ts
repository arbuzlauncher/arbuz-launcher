// JavaScript SDK, который встраивается в каждую панель плагина.
// Панель получает `window.arbuz` с RPC-вызовами к лаунчеру.
export const PANEL_SDK = String.raw`
(function () {
  'use strict';
  var cfg = window.__ARBUZ_PANEL__ || {};
  var seq = 0;
  var pending = {};
  var listeners = {};
  var ready = false;
  var queue = [];
  var readyResolve;
  var readyPromise = new Promise(function (resolve) { readyResolve = resolve; });

  function post(msg) {
    try {
      parent.postMessage(Object.assign({ type: 'arbuz-sdk' }, msg), '*');
    } catch (e) {}
  }

  function call(method, args) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      pending[id] = { resolve: resolve, reject: reject };
      var msg = { id: id, method: method, args: args === undefined ? null : args };
      if (ready) post(msg);
      else queue.push(msg);
    });
  }

  function on(name, fn) {
    (listeners[name] || (listeners[name] = [])).push(fn);
  }

  window.addEventListener('message', function (e) {
    var d = e.data;
    if (!d || typeof d !== 'object') return;
    if (d.type === 'arbuz-sdk-result' && d.id && pending[d.id]) {
      var p = pending[d.id];
      delete pending[d.id];
      if (d.error) p.reject(new Error(String(d.error)));
      else p.resolve(d.result);
      return;
    }
    if (d.type === 'arbuz-sdk-ready') {
      api.theme = d.theme || {};
      if (!ready) {
        ready = true;
        var queued = queue;
        queue = [];
        for (var qi = 0; qi < queued.length; qi++) post(queued[qi]);
      }
      readyResolve(api.theme);
      return;
    }
    if (d.type === 'arbuz-sdk-event') {
      var list = listeners[d.event] || [];
      for (var i = 0; i < list.length; i++) {
        try { list[i](d.data); } catch (err) { console.error(err); }
      }
    }
  });

  var api = {
    version: '1.0',
    apiVersion: 1,
    plugin: cfg.plugin || '',
    panel: cfg.panel || '',
    theme: {},
    ready: function (fn) {
      return readyPromise.then(function (theme) {
        if (fn) fn(theme);
        return theme;
      });
    },
    on: on,
    onCommand: on,
    call: function (command, args) {
      return call('command', { command: String(command), args: args === undefined ? null : args });
    },
    command: function (command, args) {
      return call('command', { command: String(command), args: args === undefined ? null : args });
    },
    toast: function (message) {
      return call('toast', { message: String(message) });
    },
    resize: function (height) {
      return call('resize', { height: Number(height) || 0 });
    },
    close: function () {
      return call('close', {});
    },
    open: function (panel) {
      return call('open', { panel: String(panel) });
    },
    storage: {
      get: function (key, def) {
        return call('storage.get', { key: String(key), default: def === undefined ? null : def });
      },
      set: function (key, value) {
        return call('storage.set', { key: String(key), value: value });
      },
      all: function () {
        return call('storage.all', {});
      }
    },
    config: {
      get: function (key, def) {
        return call('config.get', { key: String(key), default: def === undefined ? null : def });
      },
      set: function (key, value) {
        return call('config.set', { key: String(key), value: value });
      }
    },
    emit: function (event, data) {
      return call('emit', { event: String(event), data: data === undefined ? null : data });
    }
  };

  window.arbuz = api;
  post({ method: 'ready', args: {} });
})();
`;
