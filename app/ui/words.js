export function ownWithdrawal(reason) {
  return `Withdrawn: ${reason}`;
}

export function offerLabel(release, grants) {
  const version = release.slice(release.lastIndexOf('@') + 1);
  return `Install ${version}${grants.length ? ` — asks for: ${grants.map(grant => grant.text).join(', ')}` : ''}`;
}
