const count = document.getElementById('count');
const alert = document.getElementById('error');
const show = (value) => { count.textContent = String(value); alert.textContent = ''; };
const fail = (error) => { alert.textContent = error.message; };
const read = () => watane.call('api.get').then(show, fail);
document.getElementById('add').addEventListener('click', () => watane.call('api.add').then(show, fail));
watane.on((message) => {
  if (message.type === 'event' && message.function === 'counted') show(message.args.count);
  else if (message.type === 'changed') read();
});
