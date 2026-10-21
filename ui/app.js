(function () {
  'use strict';

  var API = '';
  var charts = {};
  var refreshTimer;
  var currentDomain = localStorage.getItem('swa_domain') || ''; // empty = all sites

  // --- Date range ---

  function formatLocal(d) {
    var y = d.getFullYear();
    var m = ('0' + (d.getMonth() + 1)).slice(-2);
    var day = ('0' + d.getDate()).slice(-2);
    return y + '-' + m + '-' + day;
  }

  function todayStr() {
    return formatLocal(new Date());
  }

  function addDays(dateStr, n) {
    var d = new Date(dateStr + 'T00:00:00');
    d.setDate(d.getDate() + n);
    return formatLocal(d);
  }

  function daysBetween(from, to) {
    return Math.round((new Date(to + 'T00:00:00') - new Date(from + 'T00:00:00')) / 86400000);
  }

  function getRange(preset) {
    var today = todayStr();
    var tomorrow = addDays(today, 1);
    if (preset === 'today') return { from: today, to: tomorrow };
    if (preset === '7d') return { from: addDays(today, -6), to: tomorrow };
    if (preset === '30d') return { from: addDays(today, -29), to: tomorrow };
    return null;
  }

  // Custom ranges are stored as {from, to} with `to` exclusive, like currentRange.
  function savedCustomRange() {
    try {
      var r = JSON.parse(localStorage.getItem('swa_range_custom'));
      if (r && /^\d{4}-\d{2}-\d{2}$/.test(r.from) && /^\d{4}-\d{2}-\d{2}$/.test(r.to) && r.from < r.to) return r;
    } catch (e) {}
    return null;
  }

  var savedPreset = localStorage.getItem('swa_range_preset') || 'today';
  var currentRange = savedPreset === 'custom' ? savedCustomRange() : getRange(savedPreset);
  if (!currentRange) {
    savedPreset = 'today';
    currentRange = getRange(savedPreset);
  }
  var currentPreset = savedPreset;

  // Convert a local-date range {from, to} to UTC ISO datetimes for API queries,
  // and include the timezone offset (minutes ahead of UTC) for server-side date grouping.
  // e.g. UTC+8 user's "2026-03-17" midnight local → "2026-03-16T16:00:00.000Z", tz_offset=480
  function apiRange(range) {
    var tz_offset = -new Date().getTimezoneOffset(); // +480 for UTC+8
    return {
      from: new Date(range.from + 'T00:00:00').toISOString(),
      to: new Date(range.to + 'T00:00:00').toISOString(),
      tz_offset: tz_offset,
      domain: currentDomain
    };
  }

  // The same number of days right before `range`. When `range` is still running
  // (it includes now), the previous window is cut at the same point in time, so
  // "today so far" compares with "yesterday up to this time".
  function previousApiRange(range) {
    var days = daysBetween(range.from, range.to);
    var start = new Date(range.from + 'T00:00:00').getTime();
    var end = Math.min(new Date(range.to + 'T00:00:00').getTime(), Date.now());
    var prevStart = new Date(addDays(range.from, -days) + 'T00:00:00');
    var prevEnd = new Date(prevStart.getTime() + Math.max(0, end - start));
    return {
      from: prevStart.toISOString(),
      to: prevEnd.toISOString(),
      tz_offset: -new Date().getTimezoneOffset(),
      domain: currentDomain
    };
  }

  // Completes "<previous value> ..." under each card, e.g. "2.8K in the previous 7 days".
  function comparisonPhrase(range) {
    var days = daysBetween(range.from, range.to);
    var running = new Date(range.to + 'T00:00:00').getTime() > Date.now();
    if (days === 1) return running ? 'by this time yesterday' : 'on the previous day';
    return 'in the previous ' + days + ' days';
  }

  // --- API helpers ---

  function api(path, params) {
    var q = new URLSearchParams(params || {}).toString();
    var url = API + path + (q ? '?' + q : '');
    return fetch(url).then(function (r) {
      if (r.status === 401) {
        window.location.href = '/login';
        return new Promise(function () {}); // never settles; the page is going away
      }
      if (!r.ok) throw new Error(path + ' returned ' + r.status);
      return r.json();
    });
  }

  function postApi(path, body) {
    return fetch(API + path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    });
  }

  // --- Chart.js defaults ---

  Chart.defaults.color = '#8b8fa3';
  Chart.defaults.borderColor = '#2e3347';
  Chart.defaults.font.family = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";

  var chartColors = ['#6c5ce7', '#00d68f', '#ffa94d', '#ff6b6b', '#74b9ff', '#fd79a8', '#a29bfe', '#fdcb6e'];

  function makeLineChart(ctx, labels, datasets) {
    if (charts[ctx.canvas.id]) charts[ctx.canvas.id].destroy();
    charts[ctx.canvas.id] = new Chart(ctx, {
      type: 'line',
      data: {
        labels: labels,
        datasets: datasets.map(function (ds, i) {
          return Object.assign({
            borderColor: chartColors[i % chartColors.length],
            backgroundColor: chartColors[i % chartColors.length] + '22',
            borderWidth: 2,
            pointRadius: labels.length > 30 ? 0 : 3,
            tension: 0.3,
            fill: true
          }, ds);
        })
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: { intersect: false, mode: 'index' },
        plugins: { legend: { display: datasets.length > 1, position: 'top' } },
        scales: {
          x: { grid: { display: false } },
          y: { beginAtZero: true, ticks: { precision: 0 } }
        }
      }
    });
  }

  function clearChart(id) {
    if (charts[id]) { charts[id].destroy(); delete charts[id]; }
  }

  // --- Number formatting ---

  function fmt(n) {
    if (n === undefined || n === null) return '-';
    if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
    if (n >= 1000) return (n / 1000).toFixed(1) + 'K';
    return n.toString();
  }

  function escHtml(s) {
    var d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  // --- Toasts and dialogs ---

  function toast(message, kind) {
    var el = document.createElement('div');
    el.className = 'toast' + (kind === 'error' ? ' toast-error' : '');
    el.textContent = message;
    document.getElementById('toasts').appendChild(el);
    setTimeout(function () {
      el.classList.add('toast-out');
      setTimeout(function () { el.remove(); }, 200);
    }, kind === 'error' ? 6000 : 3500);
  }

  // Resolves to true when the user confirms. With `requireText`, the confirm
  // button stays disabled until that exact text is typed.
  function confirmDialog(opts) {
    var dialog = document.getElementById('dialog');
    var field = document.getElementById('dialog-field');
    var input = document.getElementById('dialog-input');
    var confirm = document.getElementById('dialog-confirm');

    document.getElementById('dialog-title').textContent = opts.title;
    document.getElementById('dialog-message').textContent = opts.message || '';
    confirm.textContent = opts.confirmLabel || 'Confirm';
    confirm.className = opts.danger ? 'btn-danger-solid' : 'btn';
    field.hidden = !opts.requireText;
    input.value = '';
    input.oninput = null;
    confirm.disabled = false;
    if (opts.requireText) {
      document.getElementById('dialog-field-hint').innerHTML =
        'Type <strong>' + escHtml(opts.requireText) + '</strong> to confirm';
      confirm.disabled = true;
      input.oninput = function () { confirm.disabled = input.value !== opts.requireText; };
    }

    dialog.returnValue = '';
    dialog.showModal();
    (opts.requireText ? input : confirm).focus();
    return new Promise(function (resolve) {
      dialog.addEventListener('close', function () {
        resolve(dialog.returnValue === 'confirm');
      }, { once: true });
    });
  }

  document.getElementById('dialog-cancel').addEventListener('click', function () {
    document.getElementById('dialog').close('cancel');
  });

  // --- Panel states ---

  function panelError(el, retry) {
    el.innerHTML = '<div class="panel-error"><span>Couldn’t load this.</span> ' +
      '<button type="button" class="btn-link">Try again</button></div>';
    el.querySelector('button').addEventListener('click', retry);
  }

  // Message over a chart canvas: empty-state text, an error with a retry, or nothing.
  function chartStatus(id, message, retry) {
    var el = document.getElementById(id);
    if (!message && !retry) { el.hidden = true; el.innerHTML = ''; return; }
    el.hidden = false;
    if (retry) panelError(el, retry);
    else el.innerHTML = '<div class="empty">' + escHtml(message) + '</div>';
  }

  // Each refresh bumps loadGen; responses from an older refresh are dropped so a
  // slow request can't overwrite newer numbers after a range or site switch.
  var loadGen = 0;

  // Wraps a loader: drops stale responses and resolves to true on success,
  // false on failure (after `onError` has rendered the error state).
  function loader(fetchFn, render, onError) {
    return function (gen) {
      return fetchFn().then(function (res) {
        if (gen !== loadGen) return true;
        render(res);
        return true;
      }).catch(function () {
        if (gen !== loadGen) return true;
        onError();
        return false;
      });
    };
  }

  function retryAll() { loadAll(); }

  // --- Render functions ---

  var loadRealtime = loader(
    function () { return api('/dash/stats/realtime', { domain: currentDomain }); },
    function (res) {
      var n = res.data.active_visitors;
      document.getElementById('realtime-count').textContent = n;
      document.getElementById('realtime-label').textContent = n === 1 ? 'visitor now' : 'visitors now';
    },
    function () {}
  );

  function setKpi(key, value, prev, text, prevText) {
    document.getElementById('kpi-' + key).textContent = text;
    var delta = document.getElementById('delta-' + key);
    var prevEl = document.getElementById('prev-' + key);
    prevEl.textContent = prevText + ' ' + comparisonPhrase(currentRange);
    if (!prev) {
      delta.textContent = value ? 'New' : '';
      delta.className = 'delta delta-flat';
      delta.removeAttribute('aria-label');
      return;
    }
    var pct = Math.round((value - prev) / prev * 1000) / 10;
    var dir = pct > 0 ? 'up' : pct < 0 ? 'down' : 'flat';
    delta.textContent = (dir === 'up' ? '↑ ' : dir === 'down' ? '↓ ' : '') + Math.abs(pct).toFixed(1) + '%';
    delta.className = 'delta delta-' + dir;
    delta.setAttribute('aria-label', (dir === 'flat' ? 'No change' : (dir === 'up' ? 'Up ' : 'Down ') + Math.abs(pct).toFixed(1) + '%') +
      ', ' + prevEl.textContent);
  }

  function perVisitor(d) {
    return d.unique_visitors ? d.total_views / d.unique_visitors : 0;
  }

  var loadOverview = loader(
    function () {
      return Promise.all([
        api('/dash/stats/overview', apiRange(currentRange)),
        api('/dash/stats/overview', previousApiRange(currentRange))
      ]);
    },
    function (res) {
      var d = res[0].data;
      var p = res[1].data;
      setKpi('views', d.total_views, p.total_views, fmt(d.total_views), fmt(p.total_views));
      setKpi('visitors', d.unique_visitors, p.unique_visitors, fmt(d.unique_visitors), fmt(p.unique_visitors));
      setKpi('per-visitor', perVisitor(d), perVisitor(p),
        d.unique_visitors ? perVisitor(d).toFixed(2) : '-',
        p.unique_visitors ? perVisitor(p).toFixed(2) : '-');
      setKpi('actions', d.total_actions, p.total_actions, fmt(d.total_actions), fmt(p.total_actions));
    },
    function () {
      ['views', 'visitors', 'per-visitor', 'actions'].forEach(function (key) {
        document.getElementById('kpi-' + key).textContent = '-';
        document.getElementById('delta-' + key).textContent = '';
        document.getElementById('prev-' + key).textContent = 'Couldn’t load';
      });
    }
  );

  var loadPageviews = loader(
    function () { return api('/dash/stats/pageviews', apiRange(currentRange)); },
    function (res) {
      var data = res.data;
      if (!data.length) {
        clearChart('chart-pageviews');
        chartStatus('status-pageviews', 'No page views in this range');
        return;
      }
      chartStatus('status-pageviews');
      var labels = data.map(function (r) { return r.date; });
      var values = data.map(function (r) { return r.count; });
      var ctx = document.getElementById('chart-pageviews').getContext('2d');
      makeLineChart(ctx, labels, [{ label: 'Page views', data: values }]);
    },
    function () {
      clearChart('chart-pageviews');
      chartStatus('status-pageviews', null, retryAll);
    }
  );

  var loadPages = loader(
    function () { return api('/dash/stats/pages', Object.assign({}, apiRange(currentRange), { limit: 10 })); },
    function (res) {
      var data = res.data;
      var el = document.getElementById('table-pages');
      if (!data.length) { el.innerHTML = '<div class="empty">No data yet</div>'; return; }
      var maxViews = data[0].views;
      var html = '<table><thead><tr><th>Page</th><th>Views</th><th>Unique</th></tr></thead><tbody>';
      data.forEach(function (r) {
        var pct = maxViews ? (r.views / maxViews * 100) : 0;
        html += '<tr><td class="path-cell bar-cell"><div class="bar-fill" style="width:' + pct + '%"></div><span>' + escHtml(r.path) + '</span></td>';
        html += '<td>' + fmt(r.views) + '</td><td>' + fmt(r.unique_visitors) + '</td></tr>';
      });
      html += '</tbody></table>';
      el.innerHTML = html;
    },
    function () { panelError(document.getElementById('table-pages'), retryAll); }
  );

  var loadReferrers = loader(
    function () { return api('/dash/stats/referrers', Object.assign({}, apiRange(currentRange), { limit: 10 })); },
    function (res) {
      var data = res.data;
      var el = document.getElementById('table-referrers');
      if (!data.length) { el.innerHTML = '<div class="empty">No data yet</div>'; return; }
      var maxCount = data[0].count;
      var html = '<table><thead><tr><th>Referrer</th><th>Visits</th></tr></thead><tbody>';
      data.forEach(function (r) {
        var pct = maxCount ? (r.count / maxCount * 100) : 0;
        html += '<tr><td class="bar-cell"><div class="bar-fill" style="width:' + pct + '%"></div><span>' + escHtml(r.referrer) + '</span></td>';
        html += '<td>' + fmt(r.count) + '</td></tr>';
      });
      html += '</tbody></table>';
      el.innerHTML = html;
    },
    function () { panelError(document.getElementById('table-referrers'), retryAll); }
  );

  var loadCampaigns = loader(
    function () { return api('/dash/stats/campaigns', Object.assign({}, apiRange(currentRange), { limit: 10 })); },
    function (res) {
      var data = res.data;
      var el = document.getElementById('table-campaigns');
      if (!data.length) { el.innerHTML = '<div class="empty">No data yet</div>'; return; }
      var maxCount = data[0].count;
      var html = '<table><thead><tr><th>Source</th><th>Medium</th><th>Campaign</th><th>Visits</th></tr></thead><tbody>';
      data.forEach(function (c) {
        var pct = maxCount ? (c.count / maxCount * 100) : 0;
        html += '<tr><td class="bar-cell"><div class="bar-fill" style="width:' + pct + '%"></div><span>' + escHtml(c.source) + '</span></td>';
        html += '<td>' + escHtml(c.medium) + '</td><td>' + escHtml(c.campaign) + '</td>';
        html += '<td>' + fmt(c.count) + '</td></tr>';
      });
      html += '</tbody></table>';
      el.innerHTML = html;
    },
    function () { panelError(document.getElementById('table-campaigns'), retryAll); }
  );

  // --- Devices (browsers / OS as bar lists) ---

  var deviceTab = localStorage.getItem('swa_device_tab') === 'os' ? 'os' : 'browsers';
  var deviceData = { browsers: null, os: null };
  var deviceFailed = false;

  // rows: [{name, count}] sorted by count. Anything past `limit` is folded into "Other".
  function renderBarList(el, rows, limit) {
    if (!rows.length) { el.innerHTML = '<div class="empty">No data yet</div>'; return; }
    var total = rows.reduce(function (sum, r) { return sum + r.count; }, 0);
    if (rows.length > limit) {
      var rest = rows.slice(limit - 1).reduce(function (sum, r) { return sum + r.count; }, 0);
      rows = rows.slice(0, limit - 1).concat([{ name: 'Other', count: rest }]);
    }
    var html = '<div class="bar-list">';
    rows.forEach(function (r) {
      var pct = total ? r.count / total * 100 : 0;
      html += '<div class="bar-row"><div class="bar-row-head"><span>' + escHtml(r.name || 'Unknown') + '</span>' +
        '<span class="bar-row-num">' + fmt(r.count) + ' · ' + Math.round(pct) + '%</span></div>' +
        '<div class="bar-track"><div class="bar-track-fill" style="width:' + pct.toFixed(1) + '%"></div></div></div>';
    });
    el.innerHTML = html + '</div>';
  }

  function renderDevices() {
    document.querySelectorAll('[data-device]').forEach(function (b) {
      b.setAttribute('aria-pressed', b.getAttribute('data-device') === deviceTab ? 'true' : 'false');
    });
    var el = document.getElementById('list-devices');
    if (deviceFailed) { panelError(el, retryAll); return; }
    var data = deviceData[deviceTab];
    if (!data) return;
    renderBarList(el, data, 8);
  }

  var loadDevices = loader(
    function () {
      return Promise.all([
        api('/dash/stats/browsers', apiRange(currentRange)),
        api('/dash/stats/os', apiRange(currentRange))
      ]);
    },
    function (res) {
      deviceFailed = false;
      deviceData.browsers = res[0].data.map(function (r) { return { name: r.browser, count: r.count }; });
      deviceData.os = res[1].data.map(function (r) { return { name: r.os, count: r.count }; });
      renderDevices();
    },
    function () {
      deviceFailed = true;
      renderDevices();
    }
  );

  document.querySelectorAll('[data-device]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      deviceTab = btn.getAttribute('data-device');
      localStorage.setItem('swa_device_tab', deviceTab);
      renderDevices();
    });
  });

  // --- Actions ---

  function renderActionGroups(el, byName) {
    var groups = {};
    var list = [];
    byName.forEach(function (r) {
      var g = groups[r.name];
      if (!g) { g = groups[r.name] = { name: r.name, total: 0, labels: [] }; list.push(g); }
      g.total += r.count;
      g.labels.push({ name: r.label, count: r.count });
    });
    list.sort(function (a, b) { return b.total - a.total; });

    var html = '';
    list.forEach(function (g) {
      g.labels.sort(function (a, b) { return b.count - a.count; });
      var labels = g.labels;
      if (labels.length > 6) {
        var rest = labels.slice(5).reduce(function (sum, l) { return sum + l.count; }, 0);
        labels = labels.slice(0, 5).concat([{ name: 'other labels', count: rest, other: true }]);
      }
      html += '<div class="action-group"><div class="action-head"><span class="action-name">' + escHtml(g.name) + '</span>' +
        '<span class="action-count">' + fmt(g.total) + '</span></div>';
      // A single unlabeled action needs no breakdown
      if (labels.length > 1 || labels[0].name) {
        var bar = '';
        var legend = '';
        labels.forEach(function (l, i) {
          var color = l.other ? '#8b8fa3' : chartColors[i % chartColors.length];
          var name = l.other ? l.name : (l.name || '(no label)');
          bar += '<span style="flex:' + l.count + ' 1 0;background:' + color + '"></span>';
          legend += '<span class="action-label"><span class="action-swatch" style="background:' + color + '"></span>' +
            escHtml(name) + ' <span class="action-label-count">' + fmt(l.count) + '</span></span>';
        });
        html += '<div class="action-bar" aria-hidden="true">' + bar + '</div><div class="action-labels">' + legend + '</div>';
      }
      html += '</div>';
    });
    el.innerHTML = html;
  }

  var loadActions = loader(
    function () { return api('/dash/stats/actions', apiRange(currentRange)); },
    function (res) {
      var data = res.data;
      var el = document.getElementById('list-actions');
      var chartWrap = document.getElementById('actions-chart-wrap');
      var total = data.by_name.reduce(function (sum, r) { return sum + r.count; }, 0);
      document.getElementById('actions-total').textContent = total ? fmt(total) + ' total' : '';
      chartStatus('status-actions');

      if (!data.by_name.length) {
        clearChart('chart-actions');
        chartWrap.hidden = true;
        el.innerHTML = '<div class="empty">No actions yet. Record one with <code>window.swa.action(\'signup\')</code>.</div>';
        return;
      }

      // Build daily chart grouped by action name
      var actions = {};
      data.daily.forEach(function (r) {
        if (!actions[r.name]) actions[r.name] = {};
        actions[r.name][r.date] = r.count;
      });
      var allDates = [];
      var seen = {};
      data.daily.forEach(function (r) {
        if (!seen[r.date]) { allDates.push(r.date); seen[r.date] = true; }
      });
      var datasets = Object.keys(actions).map(function (name) {
        return {
          label: name,
          data: allDates.map(function (d) { return actions[name][d] || 0; })
        };
      });
      chartWrap.hidden = false;
      var ctx = document.getElementById('chart-actions').getContext('2d');
      makeLineChart(ctx, allDates, datasets);

      renderActionGroups(el, data.by_name);
    },
    function () {
      clearChart('chart-actions');
      document.getElementById('actions-chart-wrap').hidden = true;
      document.getElementById('actions-total').textContent = '';
      panelError(document.getElementById('list-actions'), retryAll);
    }
  );

  // --- Site sidebar ---

  var domains = [];

  function setSheetOpen(open) {
    document.body.classList.toggle('sheet-open', open);
    document.getElementById('btn-sites').setAttribute('aria-expanded', open ? 'true' : 'false');
    if (open) document.getElementById('site-filter').focus();
  }

  document.getElementById('btn-sites').addEventListener('click', function () {
    setSheetOpen(!document.body.classList.contains('sheet-open'));
  });
  document.getElementById('sheet-backdrop').addEventListener('click', function () { setSheetOpen(false); });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && document.body.classList.contains('sheet-open')) setSheetOpen(false);
  });

  function renderSiteList() {
    var list = document.getElementById('site-list');
    var filter = document.getElementById('site-filter').value.trim().toLowerCase();
    document.getElementById('site-count').textContent = domains.length ? domains.length : '';
    document.getElementById('site-switcher-label').textContent =
      isSettingsView() ? 'Settings' : (currentDomain || 'Sites');
    renderDeleteSelect();
    list.innerHTML = '';
    if (!domains.length) {
      list.innerHTML = '<div class="site-empty">No sites yet</div>';
      return;
    }
    var shown = domains.filter(function (d) {
      return d.domain.toLowerCase().indexOf(filter) !== -1;
    });
    if (!shown.length) {
      list.innerHTML = '<div class="site-empty">No matching sites</div>';
      return;
    }
    shown.forEach(function (d) {
      var btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'site-item' + (d.domain === currentDomain && !isSettingsView() ? ' active' : '');
      btn.title = d.domain;
      btn.innerHTML = '<span class="site-name">' + escHtml(d.domain) + '</span>' +
        '<span class="site-views">' + fmt(d.total_views) + '</span>';
      btn.addEventListener('click', function () {
        setSheetOpen(false);
        if (d.domain === currentDomain && !isSettingsView()) return;
        currentDomain = d.domain;
        localStorage.setItem('swa_domain', currentDomain);
        if (isSettingsView()) {
          history.pushState(null, '', location.pathname + location.search);
        }
        showView();
      });
      list.appendChild(btn);
    });
  }

  function loadDomains() {
    return api('/dash/stats/domains').then(function (res) {
      domains = res.data || [];
      // Restore previous selection or pick first
      var found = domains.some(function (d) { return d.domain === currentDomain; });
      if (!found) currentDomain = domains.length ? domains[0].domain : '';
      renderSiteList();
      renderAllowedSites();
    }).catch(function () {});
  }

  document.getElementById('site-filter').addEventListener('input', renderSiteList);

  // --- Views (dashboard / settings) ---

  function isSettingsView() {
    return location.hash === '#settings';
  }

  function showView() {
    var settings = isSettingsView();
    var empty = !currentDomain;
    document.getElementById('view-dashboard').hidden = settings;
    document.getElementById('view-settings').hidden = !settings;
    document.getElementById('site-title').textContent = currentDomain || 'No sites yet';
    document.getElementById('nav-settings').classList.toggle('active', settings);
    document.getElementById('dash-content').hidden = empty;
    document.getElementById('dash-empty').hidden = !empty;
    document.querySelectorAll('[data-dash-only]').forEach(function (el) { el.hidden = empty; });
    renderSiteList();
    if (!settings) loadAll();
  }

  window.addEventListener('hashchange', function () {
    setSheetOpen(false);
    showView();
  });

  // --- Load all ---

  var lastUpdated = 0;
  var lastFailed = false;

  function renderUpdated() {
    var el = document.getElementById('updated-text');
    el.classList.toggle('updated-error', lastFailed);
    if (lastFailed) { el.textContent = 'Some panels failed to load'; return; }
    if (!lastUpdated) { el.textContent = ''; return; }
    var secs = Math.floor((Date.now() - lastUpdated) / 1000);
    if (secs < 10) el.textContent = 'Updated just now';
    else if (secs < 60) el.textContent = 'Updated ' + secs + 's ago';
    else el.textContent = 'Updated ' + Math.floor(secs / 60) + ' min ago';
  }

  function loadAll() {
    if (!currentDomain) return;
    var gen = ++loadGen;
    var refreshBtn = document.getElementById('btn-refresh');
    refreshBtn.classList.add('spinning');
    Promise.all([
      loadRealtime(gen),
      loadOverview(gen),
      loadPageviews(gen),
      loadPages(gen),
      loadReferrers(gen),
      loadCampaigns(gen),
      loadDevices(gen),
      loadActions(gen)
    ]).then(function (results) {
      if (gen !== loadGen) return;
      refreshBtn.classList.remove('spinning');
      lastFailed = results.indexOf(false) !== -1;
      if (!lastFailed) lastUpdated = Date.now();
      renderUpdated();
    });
  }

  document.getElementById('btn-refresh').addEventListener('click', loadAll);
  setInterval(renderUpdated, 10000);

  // --- Controls ---

  function shortDate(dateStr) {
    var d = new Date(dateStr + 'T00:00:00');
    var opts = { month: 'short', day: 'numeric' };
    if (d.getFullYear() !== new Date().getFullYear()) opts.year = 'numeric';
    return d.toLocaleDateString(undefined, opts);
  }

  function renderRangeControls() {
    document.querySelectorAll('.range-controls button[data-range]').forEach(function (b) {
      b.classList.toggle('active', b.getAttribute('data-range') === currentPreset);
    });
    var custom = currentPreset === 'custom';
    var last = addDays(currentRange.to, -1);
    document.getElementById('btn-custom-toggle').classList.toggle('active', custom);
    document.getElementById('custom-label').textContent = !custom ? 'Custom'
      : (currentRange.from === last ? shortDate(last) : shortDate(currentRange.from) + ' – ' + shortDate(last));
  }

  function setPopoverOpen(open) {
    var popover = document.getElementById('custom-popover');
    popover.hidden = !open;
    document.getElementById('btn-custom-toggle').setAttribute('aria-expanded', open ? 'true' : 'false');
    if (open) {
      document.getElementById('date-from').value = currentRange.from;
      document.getElementById('date-to').value = addDays(currentRange.to, -1);
      document.getElementById('date-from').focus();
    }
  }

  document.querySelectorAll('.range-controls button[data-range]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var preset = btn.getAttribute('data-range');
      currentPreset = preset;
      currentRange = getRange(preset);
      localStorage.setItem('swa_range_preset', preset);
      setPopoverOpen(false);
      renderRangeControls();
      loadAll();
    });
  });

  document.getElementById('btn-custom-toggle').addEventListener('click', function () {
    setPopoverOpen(document.getElementById('custom-popover').hidden);
  });

  document.getElementById('custom-popover').addEventListener('keydown', function (e) {
    if (e.key === 'Escape') {
      setPopoverOpen(false);
      document.getElementById('btn-custom-toggle').focus();
    }
  });

  document.addEventListener('click', function (e) {
    var popover = document.getElementById('custom-popover');
    if (popover.hidden) return;
    if (popover.contains(e.target) || document.getElementById('btn-custom-toggle').contains(e.target)) return;
    setPopoverOpen(false);
  });

  // Open date picker when clicking anywhere on the date input
  document.getElementById('date-from').addEventListener('click', function () {
    if (this.showPicker) this.showPicker();
  });
  document.getElementById('date-to').addEventListener('click', function () {
    if (this.showPicker) this.showPicker();
  });

  document.getElementById('custom-popover').addEventListener('submit', function (e) {
    e.preventDefault();
    var from = document.getElementById('date-from').value;
    var to = document.getElementById('date-to').value;
    if (!from || !to) return;
    if (from > to) { var t = from; from = to; to = t; }
    currentPreset = 'custom';
    currentRange = { from: from, to: addDays(to, 1) };
    localStorage.setItem('swa_range_preset', 'custom');
    localStorage.setItem('swa_range_custom', JSON.stringify(currentRange));
    setPopoverOpen(false);
    renderRangeControls();
    loadAll();
  });

  // --- Empty state ---

  var trackerApi = location.protocol + '//' + location.hostname + ':3330';
  var snippet = '<script defer data-api="' + trackerApi + '" src="' + trackerApi + '/tracker.js"></script>';
  document.getElementById('tracker-snippet').textContent = snippet;

  document.getElementById('btn-copy-snippet').addEventListener('click', function () {
    var code = document.getElementById('tracker-snippet');
    function selectSnippet() {
      var range = document.createRange();
      range.selectNodeContents(code);
      var sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
      toast('Snippet selected. Press Ctrl+C or ⌘C to copy it.');
    }
    if (!navigator.clipboard) { selectSnippet(); return; }
    navigator.clipboard.writeText(snippet).then(function () {
      toast('Copied the tracker snippet');
    }, selectSnippet);
  });

  // --- Settings ---

  var allowedSites = [];
  var restrictSites = false;

  function loadSettings() {
    api('/dash/settings').then(function (res) {
      document.getElementById('toggle-localhost').checked = res.data.allow_localhost;
      restrictSites = res.data.restrict_sites;
      allowedSites = res.data.allowed_sites || [];
      renderAllowedSites();
    }).catch(function () {});
  }

  // Rough client-side mirror of the server's local-host check, only used to keep
  // local addresses out of the suggestions.
  function looksLocal(host) {
    host = host.toLowerCase();
    return !host || host === 'localhost' || /\.(localhost|local)$/.test(host) ||
      host.indexOf(':') !== -1 || /^[0-9.]+$/.test(host);
  }

  function siteAllowed(host) {
    return allowedSites.some(function (entry) {
      if (entry.indexOf('*.') === 0) {
        var base = entry.slice(1); // ".example.com"
        return host.length > base.length && host.slice(-base.length) === base;
      }
      return entry === host;
    });
  }

  function renderAllowedSites() {
    document.getElementById(restrictSites ? 'mode-list' : 'mode-any').checked = true;
    document.getElementById('allowed-panel').hidden = !restrictSites;

    var list = document.getElementById('allowed-list');
    list.innerHTML = '';
    allowedSites.forEach(function (site) {
      var row = document.createElement('div');
      row.className = 'allowed-item';
      row.innerHTML = '<span class="path-cell">' + escHtml(site) + '</span>';
      var remove = document.createElement('button');
      remove.type = 'button';
      remove.className = 'btn-remove';
      remove.textContent = 'Remove';
      remove.addEventListener('click', function () {
        saveAllowedSites(allowedSites.filter(function (s) { return s !== site; }));
      });
      row.appendChild(remove);
      list.appendChild(row);
    });

    // Tracked public sites that the list doesn't cover yet: one click to add
    var suggested = document.getElementById('suggested-sites');
    var missing = domains.filter(function (d) {
      return !looksLocal(d.domain) && !siteAllowed(d.domain.toLowerCase());
    });
    suggested.innerHTML = '';
    suggested.hidden = !missing.length;
    if (missing.length) {
      suggested.appendChild(document.createTextNode('Tracked sites not on the list:'));
      missing.forEach(function (d) {
        var chip = document.createElement('button');
        chip.type = 'button';
        chip.className = 'chip';
        chip.textContent = '+ ' + d.domain;
        chip.addEventListener('click', function () {
          saveAllowedSites(allowedSites.concat([d.domain]));
        });
        suggested.appendChild(chip);
      });
    }

    var warning = document.getElementById('allowed-warning');
    warning.hidden = allowedSites.length > 0;
    warning.textContent = 'The list is empty, so events from every public website are being dropped.';
  }

  function saveSiteSettings(body) {
    return postApi('/dash/settings', body).then(function (r) {
      if (!r.ok) toast('Failed to save settings.', 'error');
      return loadSettings();
    }).catch(function () {
      toast('Failed to save settings.', 'error');
      loadSettings();
    });
  }

  function saveAllowedSites(next) {
    return saveSiteSettings({ allowed_sites: next });
  }

  document.querySelectorAll('input[name="site-mode"]').forEach(function (radio) {
    radio.addEventListener('change', function () {
      restrictSites = this.value === 'list';
      renderAllowedSites();
      saveSiteSettings({ restrict_sites: restrictSites });
    });
  });

  document.getElementById('allowed-form').addEventListener('submit', function (e) {
    e.preventDefault();
    var input = document.getElementById('allowed-input');
    var entries = input.value.split(/[\s,;]+/).filter(Boolean);
    if (!entries.length) return;
    saveAllowedSites(allowedSites.concat(entries)).then(function () { input.value = ''; });
  });

  function renderDeleteSelect() {
    var select = document.getElementById('delete-site');
    var keep = select.value || currentDomain;
    select.innerHTML = '';
    domains.forEach(function (d) {
      var opt = document.createElement('option');
      opt.value = d.domain;
      opt.textContent = d.domain;
      select.appendChild(opt);
    });
    if (domains.some(function (d) { return d.domain === keep; })) select.value = keep;
    select.disabled = !domains.length;
    document.getElementById('btn-delete-domain').disabled = !domains.length;
  }

  document.getElementById('btn-delete-domain').addEventListener('click', function () {
    var domain = document.getElementById('delete-site').value;
    if (!domain) return;
    var info = domains.filter(function (d) { return d.domain === domain; })[0];
    var views = info ? info.total_views.toLocaleString() + ' page views' : 'all page views';
    confirmDialog({
      title: 'Delete all data for ' + domain + '?',
      message: 'This permanently removes ' + views + ' and every action recorded for ' + domain + '. It can’t be undone.',
      confirmLabel: 'Delete data',
      danger: true,
      requireText: domain
    }).then(function (ok) {
      if (!ok) return;
      postApi('/dash/data/delete', { domain: domain }).then(function (r) {
        if (!r.ok) { toast('Delete failed.', 'error'); return; }
        if (domain === currentDomain) {
          currentDomain = '';
          localStorage.removeItem('swa_domain');
        }
        loadDomains().then(showView);
        toast('Deleted all data for ' + domain);
      }).catch(function () { toast('Delete failed.', 'error'); });
    });
  });

  document.getElementById('toggle-localhost').addEventListener('change', function () {
    var checked = this.checked;
    function revert() {
      document.getElementById('toggle-localhost').checked = !checked;
      toast('Failed to save settings.', 'error');
    }
    postApi('/dash/settings', { allow_localhost: checked }).then(function (r) {
      if (!r.ok) revert();
    }).catch(revert);
  });

  // --- Init ---
  renderRangeControls();
  renderDevices();
  loadDomains().then(showView);
  loadSettings();

  // Auto-refresh every 60s. Domains are reloaded too, so new sites (or the first
  // one, on the empty state) show up without a page reload.
  refreshTimer = setInterval(function () {
    var before = currentDomain;
    loadDomains().then(function () {
      if (currentDomain !== before) showView();
      else if (!isSettingsView()) loadAll();
    });
  }, 60000);

  // Logout
  document.getElementById('btn-logout').addEventListener('click', function () {
    confirmDialog({ title: 'Log out of SWA Analytics?', confirmLabel: 'Log out' }).then(function (ok) {
      if (!ok) return;
      fetch('/auth/logout', { method: 'POST' }).then(function () {
        window.location.href = '/login';
      });
    });
  });
})();
