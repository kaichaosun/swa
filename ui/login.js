(function () {
  'use strict';

  var isRegister = false;
  var form = document.getElementById('auth-form');
  var btn = document.getElementById('btn-submit');
  var errorEl = document.getElementById('error-msg');
  var subtitle = document.getElementById('form-subtitle');
  var toggleLink = document.getElementById('toggle-link');
  var toggleText = document.getElementById('toggle-text');

  function showError(msg) {
    errorEl.textContent = msg;
    errorEl.style.display = 'block';
  }

  function hideError() {
    errorEl.style.display = 'none';
  }

  toggleLink.addEventListener('click', function (e) {
    e.preventDefault();
    isRegister = !isRegister;
    hideError();
    if (isRegister) {
      btn.textContent = 'Create Account';
      subtitle.textContent = 'Create your admin account';
      toggleText.textContent = 'Already have an account? ';
      toggleLink.textContent = 'Sign in';
    } else {
      btn.textContent = 'Sign In';
      subtitle.textContent = 'Sign in to your dashboard';
      toggleText.textContent = 'First time? ';
      toggleLink.textContent = 'Create account';
    }
  });

  form.addEventListener('submit', function (e) {
    e.preventDefault();
    hideError();

    var email = document.getElementById('email').value;
    var password = document.getElementById('password').value;
    var endpoint = isRegister ? '/auth/register' : '/auth/login';

    btn.disabled = true;
    btn.textContent = isRegister ? 'Creating...' : 'Signing in...';

    fetch(endpoint, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email: email, password: password })
    })
      .then(function (r) { return r.json().then(function (d) { return { status: r.status, data: d }; }); })
      .then(function (res) {
        if (res.data.success) {
          if (isRegister) {
            // After registration, auto-login
            return fetch('/auth/login', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ email: email, password: password })
            })
              .then(function (r) { return r.json(); })
              .then(function (d) {
                if (d.success) {
                  window.location.href = '/';
                } else {
                  showError(d.message);
                }
              });
          } else {
            window.location.href = '/';
          }
        } else {
          showError(res.data.message);
        }
      })
      .catch(function () {
        showError('Network error. Please try again.');
      })
      .finally(function () {
        btn.disabled = false;
        btn.textContent = isRegister ? 'Create Account' : 'Sign In';
      });
  });
})();
