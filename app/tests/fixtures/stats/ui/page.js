watane.call('api.result').then(value => document.querySelector('#result').textContent = String(value)).catch(error => document.querySelector('#result').textContent = error.message);
