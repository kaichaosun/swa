(function () {
  'use strict';

  var API = '';
  var charts = {};
  var refreshTimer;

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

  function getRange(preset) {
    var today = todayStr();
    var tomorrow = addDays(today, 1);
    if (preset === 'today') return { from: today, to: tomorrow };
    if (preset === '7d') return { from: addDays(today, -6), to: tomorrow };
    if (preset === '30d') return { from: addDays(today, -29), to: tomorrow };
    return null;
  }

  var currentRange = getRange('today');

  // --- API helpers ---

  function api(path, params) {
    var q = new URLSearchParams(params || {}).toString();
    var url = API + path + (q ? '?' + q : '');
    return fetch(url).then(function (r) { return r.json(); });
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

  function makePieChart(ctx, labels, data) {
    if (charts[ctx.canvas.id]) charts[ctx.canvas.id].destroy();
    charts[ctx.canvas.id] = new Chart(ctx, {
      type: 'doughnut',
      data: {
        labels: labels,
        datasets: [{
          data: data,
          backgroundColor: chartColors.slice(0, data.length),
          borderWidth: 0
        }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: { position: 'right', labels: { boxWidth: 12, padding: 12 } }
        }
      }
    });
  }

  // --- Number formatting ---

  function fmt(n) {
    if (n === undefined || n === null) return '-';
    if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
    if (n >= 1000) return (n / 1000).toFixed(1) + 'K';
    return n.toString();
  }

  // --- Render functions ---

  function loadRealtime() {
    api('/api/stats/realtime').then(function (res) {
      document.getElementById('realtime-count').textContent = res.data.active_visitors;
    }).catch(function () {});
  }

  function loadOverview() {
    api('/api/stats/overview', currentRange).then(function (res) {
      var d = res.data;
      document.getElementById('total-views').textContent = fmt(d.total_views);
      document.getElementById('unique-visitors').textContent = fmt(d.unique_visitors);
      document.getElementById('avg-views').textContent = d.avg_views_per_day.toFixed(1);
      document.getElementById('total-downloads').textContent = fmt(d.total_downloads);
    }).catch(function () {});
  }

  function loadPageviews() {
    api('/api/stats/pageviews', currentRange).then(function (res) {
      var data = res.data;
      var labels = data.map(function (r) { return r.date; });
      var values = data.map(function (r) { return r.count; });
      var ctx = document.getElementById('chart-pageviews').getContext('2d');
      makeLineChart(ctx, labels, [{ label: 'Page Views', data: values }]);
    }).catch(function () {});
  }

  function loadPages() {
    api('/api/stats/pages', Object.assign({}, currentRange, { limit: 10 })).then(function (res) {
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
    }).catch(function () {});
  }

  function loadReferrers() {
    api('/api/stats/referrers', Object.assign({}, currentRange, { limit: 10 })).then(function (res) {
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
    }).catch(function () {});
  }

  function loadBrowsers() {
    api('/api/stats/browsers', currentRange).then(function (res) {
      var data = res.data;
      if (!data.length) return;
      var ctx = document.getElementById('chart-browsers').getContext('2d');
      makePieChart(ctx, data.map(function (r) { return r.browser; }), data.map(function (r) { return r.count; }));
    }).catch(function () {});
  }

  function loadOS() {
    api('/api/stats/os', currentRange).then(function (res) {
      var data = res.data;
      if (!data.length) return;
      var ctx = document.getElementById('chart-os').getContext('2d');
      makePieChart(ctx, data.map(function (r) { return r.os; }), data.map(function (r) { return r.count; }));
    }).catch(function () {});
  }

  function loadDownloads() {
    api('/api/stats/downloads', currentRange).then(function (res) {
      var data = res.data;
      // Build daily chart grouped by app
      var apps = {};
      data.daily.forEach(function (r) {
        if (!apps[r.app_name]) apps[r.app_name] = {};
        apps[r.app_name][r.date] = r.count;
      });
      var allDates = [];
      var seen = {};
      data.daily.forEach(function (r) {
        if (!seen[r.date]) { allDates.push(r.date); seen[r.date] = true; }
      });
      var datasets = Object.keys(apps).map(function (name) {
        return {
          label: name,
          data: allDates.map(function (d) { return apps[name][d] || 0; })
        };
      });
      var ctx = document.getElementById('chart-downloads').getContext('2d');
      if (allDates.length) {
        makeLineChart(ctx, allDates, datasets);
      }

      // App table
      var el = document.getElementById('table-downloads');
      if (!data.by_app.length) { el.innerHTML = '<div class="empty">No downloads yet</div>'; return; }
      var html = '<table><thead><tr><th>App</th><th>Platform</th><th>Downloads</th></tr></thead><tbody>';
      data.by_app.forEach(function (r) {
        html += '<tr><td>' + escHtml(r.app_name) + '</td><td>' + escHtml(r.platform) + '</td><td>' + fmt(r.count) + '</td></tr>';
      });
      html += '</tbody></table>';
      el.innerHTML = html;
    }).catch(function () {});
  }

  function escHtml(s) {
    var d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  // --- Load all ---

  function loadAll() {
    loadRealtime();
    loadOverview();
    loadPageviews();
    loadPages();
    loadReferrers();
    loadBrowsers();
    loadOS();
    loadDownloads();
  }

  // --- Controls ---

  document.querySelectorAll('.controls button[data-range]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      document.querySelectorAll('.controls button[data-range]').forEach(function (b) { b.classList.remove('active'); });
      btn.classList.add('active');
      currentRange = getRange(btn.getAttribute('data-range'));
      document.getElementById('date-from').value = currentRange.from;
      document.getElementById('date-to').value = addDays(currentRange.to, -1);
      loadAll();
    });
  });

  // Open date picker when clicking anywhere on the date input
  document.getElementById('date-from').addEventListener('click', function () {
    if (this.showPicker) this.showPicker();
  });
  document.getElementById('date-to').addEventListener('click', function () {
    if (this.showPicker) this.showPicker();
  });

  document.getElementById('btn-custom').addEventListener('click', function () {
    var from = document.getElementById('date-from').value;
    var to = document.getElementById('date-to').value;
    if (from && to) {
      document.querySelectorAll('.controls button[data-range]').forEach(function (b) { b.classList.remove('active'); });
      currentRange = { from: from, to: addDays(to, 1) };
      loadAll();
    }
  });

  // --- Init ---
  loadAll();

  // Auto-refresh every 60s
  refreshTimer = setInterval(loadAll, 60000);
})();
