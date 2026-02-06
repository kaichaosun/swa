(function() {
  'use strict';
  if (navigator.doNotTrack === '1') return;

  var script = document.currentScript;
  var api = script && script.getAttribute('data-api');
  if (!api) return;

  function getBrowser() {
    var ua = navigator.userAgent;
    if (ua.indexOf('Firefox') > -1) return 'Firefox';
    if (ua.indexOf('Edg') > -1) return 'Edge';
    if (ua.indexOf('Chrome') > -1) return 'Chrome';
    if (ua.indexOf('Safari') > -1) return 'Safari';
    if (ua.indexOf('Opera') > -1 || ua.indexOf('OPR') > -1) return 'Opera';
    return 'Other';
  }

  function getOS() {
    var ua = navigator.userAgent;
    if (ua.indexOf('Win') > -1) return 'Windows';
    if (ua.indexOf('Mac') > -1) return 'macOS';
    if (ua.indexOf('Linux') > -1) return 'Linux';
    if (ua.indexOf('Android') > -1) return 'Android';
    if (ua.indexOf('iPhone') > -1 || ua.indexOf('iPad') > -1) return 'iOS';
    return 'Other';
  }

  function hash(str) {
    var h = 0;
    for (var i = 0; i < str.length; i++) {
      h = ((h << 5) - h) + str.charCodeAt(i);
      h |= 0;
    }
    return Math.abs(h).toString(36);
  }

  function getVisitorId() {
    var d = new Date().toISOString().slice(0, 10);
    var s = navigator.userAgent + screen.width + 'x' + screen.height +
      Intl.DateTimeFormat().resolvedOptions().timeZone + d;
    return hash(s);
  }

  function send() {
    var loc = window.location;
    var data = {
      domain: loc.hostname,
      path: loc.pathname,
      referrer: document.referrer || '',
      browser: getBrowser(),
      os: getOS(),
      screen: screen.width + 'x' + screen.height,
      visitor_id: getVisitorId()
    };
    var url = api.replace(/\/$/, '') + '/api/event';
    if (navigator.sendBeacon) {
      navigator.sendBeacon(url, JSON.stringify(data));
    } else {
      var xhr = new XMLHttpRequest();
      xhr.open('POST', url, true);
      xhr.setRequestHeader('Content-Type', 'application/json');
      xhr.send(JSON.stringify(data));
    }
  }

  if (document.readyState === 'complete') {
    send();
  } else {
    window.addEventListener('load', send);
  }
})();
