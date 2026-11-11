watane.on(async () => { document.getElementById('books').textContent = JSON.stringify([await watane.call('api.list'), await watane.call('api.received')]); });
