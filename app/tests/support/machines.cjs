const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const nodes = new Map();
function element() {
  return {textContent:'', children:[], dataset:{}, disabled:false, checked:false, value:'code',
    append(e) {this.children.push(e);}, replaceChildren() {this.children=[];},
    click() {if (!this.disabled) return this.onclick?.();},
    querySelectorAll() {return this.children.flatMap(e => e.children);}, querySelector() {return element();}, showModal() {}, close() {}, focus() {}};
}
const $ = s => {if (!nodes.has(s)) nodes.set(s, element()); return nodes.get(s);};
let view = {on:true, node:'self', machine:'anchor', anchor:'self', machines:[], relays:['https://relay'], problems:[], dht:false};
let joins = 0, failJoin;
const context = {document:{querySelector:$, createElement:element}, navigator:{},
  call:async cmd => cmd === 'machines' ? view : {identity:{signed:true}},
  Promise, Date, JSON};
vm.createContext(context);
vm.runInContext(fs.readFileSync('ui/machines.js','utf8').replace(/^import .*\n/, '').replaceAll('export ', ''), context);
context.setupMachines({showError() {}, action:cmd => {
  if (cmd === 'machine_join') {joins++; return new Promise((_, reject) => {failJoin=reject;});}
  return Promise.resolve();
}});
(async () => {
  const ac = process.argv[2];
  await context.machinesView({identity:null});
  if (ac === 'ac1') {
    for (const [machine, anchor, expected] of [
      ['anchor','self','This machine: self · anchor'],
      ['full','other','This machine: self · full of other'],
      ['light','other','This machine: self · light of other'],
      ['alone',null,'This machine: self · alone']]) {
      view = {...view, machine, anchor}; await context.machinesView({identity:null});
      assert.equal($('#machine-info').textContent, expected);
    }
  }
  if (ac === 'ac2') {
    view.machines = [{node:'self', role:'full', you:true, anchor:true}, {node:'other', role:'light', online:true}];
    await context.machinesView({identity:null});
    const [self, other] = $('#machine-list').children;
    assert.match(self.textContent, /this machine/);
    assert.doesNotMatch(self.textContent, /Last seen|online|offline/);
    assert.equal(other.textContent, 'other · light · online · Last seen: unknown · Last pull: unknown');
  }
  if (ac === 'ac3') {
    const click = $('#join').click();
    assert.equal($('#join').disabled, true, 'disabled synchronously');
    $('#join').click();
    for (let i=0; i<10; i++) await Promise.resolve();
    assert.equal(joins, 1);
    await context.machinesView({identity:null});
    assert.equal($('#join').disabled, true, 'refresh cannot unlock in-flight join');
    failJoin(new Error('refused'));
    await click;
    for (let i=0; i<10; i++) await Promise.resolve();
    assert.equal($('#join').disabled, false, 'failure enables retry');
  }
  if (ac === 'ac4') {
    for (const on of [false,true,false,true]) {
      view.on=on; await context.machinesView({identity:null});
      for (const selector of ['#relay-url','#relay-add','#dht-on']) assert.equal($(selector).disabled, !on, selector);
      assert.equal($('#relay-list').children[0].children[0].disabled, !on, 'Remove relay');
    }
  }
})().catch(e => {console.error(e); process.exitCode=1;});
