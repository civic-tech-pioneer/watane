const menu = document.getElementById('menu');
const toolbar = document.getElementById('toolbar');
const status = document.getElementById('status');
const alert = document.getElementById('error');
const fail = (error) => { alert.textContent = error.message; };

const fill = (box, items, empty) => {
  box.replaceChildren(...items.map((item) => {
    const button = document.createElement('button');
    button.textContent = item.label;
    button.addEventListener('click', () => watane.call('ui.click', { id: item.id }).catch(fail));
    return button;
  }));
  if (!items.length) box.innerHTML = `<span class="empty">${empty}</span>`;
};

const read = async () => {
  try {
    const items = await watane.call('registry.list');
    fill(menu, items.filter((i) => i.place === 'menu'), 'No menu items');
    fill(toolbar, items.filter((i) => i.place === 'toolbar'), 'No toolbar items');
    status.textContent = (await watane.call('registry.status')) ?? 'Ready';
    alert.textContent = '';
  } catch (error) { fail(error); }
};

// Contributors answer announce inside this call: delivery is synchronous, so the list is complete when it returns.
watane.call('ui.start').then(read, fail);
watane.on((message) => { if (message.type === 'changed') read(); });
