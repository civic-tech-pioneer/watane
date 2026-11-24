const field = id => document.getElementById(id);
const result = field('result');
const show = value => { result.textContent = JSON.stringify(value); };
const failed = error => show({code: error.code, message: error.message});
let request = null;
const read = async () => {
  try {
    field('mods-result').textContent = JSON.stringify(await watane.call('api.mods'));
    if (request) show(await watane.call('api.request', {id: request}));
  } catch (error) { field('mods-result').textContent = JSON.stringify({code: error.code, message: error.message}); }
};
for (const button of document.querySelectorAll('button')) {
  button.onclick = async () => {
    request = null;
    result.textContent = '';
    try {
      const fn = button.id;
      let args;
      if (fn === 'request-install') args = {release: field('release').value, link: field('link').value || null};
      else if (fn === 'request') args = {id: field('request-id').value};
      else if (!['mods', 'who'].includes(fn)) {
        args = {local: field('local').value};
        if (['withdraw', 'request-grant'].includes(fn)) args.grant = JSON.parse(field('grant').value);
        if (fn === 'request-remove') args['delete-data'] = field('delete-data').checked;
      }
      const value = await watane.call('api.' + fn, args);
      show(value);
      if (fn.startsWith('request-') && value.ok) {
        request = value.ok;
        field('request-id').value = request;
        await read();
      }
    } catch (error) { failed(error); }
  };
}
watane.on(read);
window.addEventListener('watane:grant-changed', read);
read();
