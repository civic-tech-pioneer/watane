const books = document.getElementById('books');
const error = document.getElementById('error');
const read = () => watane.call('books.list').then(value => { books.textContent = JSON.stringify(value); error.textContent = ''; }, e => { error.textContent = e.message; });
document.getElementById('add').onclick = () => watane.call('books.add', {title: document.getElementById('title').value}).then(read, e => { error.textContent = e.message; });
watane.on(read);
