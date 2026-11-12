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
