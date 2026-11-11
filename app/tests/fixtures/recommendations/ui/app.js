const books = document.getElementById('books');
const error = document.createElement('p'); error.id = 'error'; document.body.append(error);
const read = async () => {
  try { books.textContent = JSON.stringify([await watane.call('api.list'), await watane.call('api.received')]); error.textContent = ''; }
  catch (e) { error.textContent = e.code === 'not_granted' ? 'not granted' : e.message; }
};
watane.on(read);
