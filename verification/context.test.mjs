import test from 'node:test';
import assert from 'node:assert/strict';
import {directChild,scanEntryIds,contextActions} from '../src/lib/context.ts';
const e=(id,name,is_dir=false)=>({id,name,is_dir,size:0,compressed_size:0,is_encrypted:false});
const options={archive:true,canExtract:true,canDelete:false,canScan:true,canCreate:true,busy:false};
test('current folder is excluded from its own listing and direct children are normalized',()=>{
  assert.equal(directChild(e('0','paper/',true),'paper'),false);
  assert.equal(directChild(e('1','paper/main.tex'),'paper'),true);
  assert.equal(directChild(e('2','paper/figures/',true),'paper'),true);
  assert.equal(directChild(e('3','paper/figures/a.svg'),'paper'),false);
  assert.equal(directChild(e('0','paper/',true),''),true);
});
test('selected folders expand to files once, without widening an empty selection',()=>{
  const list=[e('0','paper/',true),e('1','paper/a.txt'),e('2','paper/sub/b.txt'),e('3','other.txt')];
  assert.deepEqual(scanEntryIds(list,[list[0],list[1]]),['1','2']);
  assert.deepEqual(scanEntryIds(list,[]),[]);
  assert.deepEqual(scanEntryIds(list,[e('virtual','empty/',true)]),[]);
});
test('context actions adapt to files, folders, background, support and busy state',()=>{
  const file=contextActions([e('1','a.txt')],options);
  assert.equal(file[0].id,'preview');assert.ok(file.some(a=>a.label==='Scan selected file'));
  assert.equal(file.find(a=>a.id==='delete').disabled,true);
  const folder=contextActions([e('0','paper/',true)],options);
  assert.equal(folder[0].id,'openFolder');assert.ok(folder.some(a=>a.label==='Scan folder'));
  const multi=contextActions([e('1','a.txt'),e('2','b.txt')],options);
  assert.ok(!multi.some(a=>a.id==='preview'));assert.ok(multi.some(a=>a.label==='Copy filenames'));
  assert.ok(contextActions([] ,options).some(a=>a.id==='scanAll'));
  assert.equal(contextActions([e('1','a.txt')],{...options,busy:true}).find(a=>a.id==='scanSelection').disabled,true);
});
