const items = document.getElementById('items');
const commands = document.getElementById('commands');
const error = document.getElementById('error');
const failed = e => { error.textContent = e.message; };
const read = () => watane.call('api.items').then(value => {
  items.textContent = JSON.stringify(value);
  commands.replaceChildren();
  for (const id of value) {
    const button = document.createElement('button');
    button.textContent = id;
    button.onclick = () => watane.call('api.click', {id}).then(() => {
      document.getElementById('clicked').textContent = id;
      error.textContent = '';
    }, failed);
    commands.append(button);
  }
  error.textContent = '';
}, failed);
document.getElementById('refresh').onclick = read;
read();
