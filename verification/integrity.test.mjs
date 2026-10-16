import test from 'node:test';
import assert from 'node:assert/strict';
import {testOutcome} from '../src/lib/integrity.ts';
const receipt={operation:'test',written:['a'],skipped:[],bytes_processed:4,message:'Checked'};
test('successful receipt is required for an integrity pass',()=>{
 assert.equal(testOutcome(null,'').state,'ready');assert.equal(testOutcome(receipt,'').state,'passed');
 assert.equal(testOutcome({...receipt,skipped:['b']},'').state,'incomplete');
});
test('password and cancelled tests cannot be presented as corruption or success',()=>{
 assert.equal(testOutcome(null,'Invalid password').state,'password');
 assert.equal(testOutcome(receipt,'Operation cancelled').state,'cancelled');
 assert.equal(testOutcome(null,'CRC mismatch',true).state,'incomplete');
 assert.equal(testOutcome(null,'CRC mismatch',false).state,'failed');
 assert.equal(testOutcome(null,'MRAR block SHA-256 mismatch').state,'failed');
});
test('resource, identity and I/O errors remain incomplete',()=>{
 for(const failure of ['Archive changed during the test','Permission denied: /data/crc.zip','Size exceeds limit','Unsupported compression'])assert.equal(testOutcome(null,failure).state,'incomplete');
});
