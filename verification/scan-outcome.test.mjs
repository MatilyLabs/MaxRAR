import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { scanOutcome } from '../src/lib/scan.ts';

const fixture=JSON.parse(readFileSync(new URL('./fixtures/scan/scan-report.json',import.meta.url)));
function clean(){const r=structuredClone(fixture);r.rule_findings=[];r.structural_findings=[];return r;}

test('real harmless demo reports caution, separates its test marker',()=>{
  const outcome=scanOutcome(fixture);
  assert.equal(outcome.state,'attention');assert.equal(outcome.issues.length,1);
  assert.equal(outcome.notes[0].severity,'test');
});
test('green requires complete coverage and no review findings',()=>{
  assert.equal(scanOutcome(clean()).state,'clear');
  for(const patch of [{complete:false},{skipped_files:1},{truncated_files:1},{opaque_containers:1},{scanned_files:0},{total_files:0,scanned_files:0}]){
    const r=clean();Object.assign(r.coverage,patch);assert.equal(scanOutcome(r).state,'incomplete');
  }
});
test('partial coverage never hides warning or unsafe entries',()=>{
  const r=structuredClone(fixture);r.coverage.complete=false;
  assert.equal(scanOutcome(r).state,'attention');assert.equal(scanOutcome(r).complete,false);
  r.structural_findings.push({code:'unsafe_path',severity:'error',message:'Outside destination'});
  assert.equal(scanOutcome(r).state,'blocked');assert.equal(scanOutcome(r).complete,false);
});
test('benign detector tests and informational file types do not become threats',()=>{
  const r=clean();r.rule_findings=fixture.rule_findings.filter(f=>f.severity==='test');
  r.structural_findings=[{code:'executable_binary',severity:'info',message:'File type only'}];
  const outcome=scanOutcome(r);assert.equal(outcome.state,'clear');assert.equal(outcome.issues.length,0);assert.equal(outcome.notes.length,2);
});
