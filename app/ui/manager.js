export function connections(mod, mods) {
  const installed = new Set(mods.map(m => m.local));
  const targets = (source, kind) => [...new Set((source.links || []).filter(l => l.kind === kind && l.granted && installed.has(l.provider)).map(l => l.provider))];
  return {
    uses: targets(mod, 'call'), addsTo: targets(mod, 'subscribe'),
    usedBy: mods.filter(m => targets(m, 'call').includes(mod.local)).map(m => m.local),
    extendedBy: mods.filter(m => targets(m, 'subscribe').includes(mod.local)).map(m => m.local)
  };
}

export function dependents(mod, mods) {
  const enabled = new Set(mods.filter(m => m.enabled).map(m => m.local));
  const {usedBy, extendedBy} = connections(mod, mods);
  return {usedBy: usedBy.filter(l => enabled.has(l)), extendedBy: extendedBy.filter(l => enabled.has(l))};
}
export function matchesLibrary(mod, text, source) {
  return (!source || (source === 'published' ? !!mod.publisher : !mod.publisher)) &&
    text.trim().toLocaleLowerCase().split(/\s+/).every(word => `${mod.name} ${mod.publisher?.name || ''} ${mod.publisher?.fingerprint || ''}`.toLocaleLowerCase().includes(word));
}

export function updateOffers(mods) {
  return mods.filter(m => (m.offer && m.offerGrants.length) ||
    m.notice === "The publisher's channel is compromised" || m.notice?.startsWith('Withdrawn by the publisher: '));
}
