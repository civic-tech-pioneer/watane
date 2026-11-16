export function ownWithdrawal(reason) {
  return `Withdrawn: ${reason}`;
}

export function offerLabel(release, grants) {
  const version = release.slice(release.lastIndexOf('@') + 1);
  return `Install ${version}${grants.length ? ` — asks for: ${grants.map(grant => grant.text).join(', ')}` : ''}`;
}

export function signTitle(statement) {
  return {profile: 'Sign your profile', release: 'Sign a release', withdraw: 'Sign a withdrawal',
    list: 'List in your index', include: 'Include in your index', unlist: 'End a listing'}[statement];
}

export function partyLabel(party) {
  if (party.you) return 'you';
  return party.name ? `${party.name} (${party.fingerprint})` : party.fingerprint;
}

export function modCount(count) {
  return `${count} ${count === 1 ? 'mod' : 'mods'}`;
}

export function checkedLabel(hit, now = Date.now()) {
  if (hit.uncheckedSince !== null) return `could not check since ${new Date(hit.uncheckedSince).toLocaleString()}`;
  if (hit.checkedAt === null) return '';
  const minutes = Math.max(0, Math.floor((now - hit.checkedAt) / 60000));
  return minutes === 0 ? 'checked just now' : `checked ${minutes} min ago`;
}

export function viaLabel(via) {
  if (via.kind === 'own') return 'Your release';
  if (via.kind === 'followed') return 'Followed channel';
  return `listed by ${partyLabel(via.index)}${via.through ? ` through ${partyLabel(via.through)}` : ''}${via.note ? ` · ${via.note}` : ''}`;
}

export function emptySearch() { return 'No mods found'; }
export function derivedAction(hit) { return hit.installed ? 'Installed' : hit.verified ? 'Install' : ''; }
export function viaLabels(vias) {
  const sameIndex = (a, b) => a.kind === 'index' && b.kind === 'index' && JSON.stringify(a.index) === JSON.stringify(b.index);
  return vias.filter(via => via.through || !vias.some(other => other.through && sameIndex(via, other)))
    .map(via => {
      const notes = [via.note, ...vias.filter(other => via.through && !other.through && sameIndex(via, other)).map(other => other.note)];
      return viaLabel({...via, note: [...new Set(notes.filter(Boolean))].join(' · ')});
    });
}
export function actionErrors(render) {
  let latest = 0;
  return {
    begin() { render(''); return ++latest; },
    current() { return latest; },
    show(error, id = latest) {
      if (id === latest) render(error ? error.message || String(error) : '');
    }
  };
}

export function visibleIndexes(indexes) {
  return indexes.filter(index => !index.own || indexes.some(other => !other.own) || index.listings.length || index.included.length);
}

export function askerLabel(asker) {
  return asker.publisher ? `${asker.name} by ${partyLabel(asker.publisher)}` : `${asker.name} (local)`;
}

export function manageTitle(request) {
  const target = request.target.name;
  const action = {enable: `turn on ${target}`, grant: `give ${target} a grant`,
    remove: `remove ${target}`, remove_data: `remove ${target} and delete its data`}[request.action];
  return `${request.asked_by.name} asks to ${action}`;
}

export function dependencyWarning(verb, name, users, extensions) {
  const sentences = [];
  if (users.length) sentences.push(`These mods use it: ${users.join(', ')}.`);
  if (extensions.length) sentences.push(`These mods extend it: ${extensions.join(', ')}.`);
  return sentences.length ? `${verb} ${name}? ${sentences.join(' ')}` : '';
}

export function monogram(mod, size) {
  const node = document.createElement('span');
  const slug = mod.slug || mod.local;
  let sum = 0;
  for (let i = 0; i < slug.length; i++) sum += slug.charCodeAt(i);
  node.className = `mono mono-${sum % 8} mono-${size}`;
  node.textContent = Array.from(mod.name || slug)[0].toLocaleUpperCase();
  node.setAttribute('aria-hidden', 'true');
  return node;
}
