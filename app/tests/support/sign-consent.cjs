const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const warning = 'After this, this home cannot join your other machines.';
(async () => {
  for (const first of [true, false]) {
    const nodes = new Map();
    const element = () => ({textContent:'', children:[], dataset:{}, append(e) {this.children.push(e);}, focus() {}});
    const $ = s => {if (!nodes.has(s)) nodes.set(s, element()); return nodes.get(s);};
    const context = {document:{querySelector:$, createElement:element, hasFocus:() => true, body:element()},
      window:{addEventListener() {}}, setTimeout() {}, clearTimeout() {},
      signTitle:() => 'Sign a release', partyLabel:() => 'you',
      call:async () => ({id:'id', mark:'mark', request:{kind:'sign', statement:'release', signer:{},
        details:first ? [{label:'', value:warning}] : []}})};
    vm.createContext(context);
    const source = fs.readFileSync('ui/consent.js', 'utf8').replace(/^import .*\n/gm, '');
    await vm.runInContext('(async () => {'+source+'})()', context);
    assert.equal($('main').children.some(e => e.textContent === warning), first);
    assert.equal(context.document.body.dataset.ready, 'true');
  }
})().catch(e => {console.error(e); process.exitCode=1;});
