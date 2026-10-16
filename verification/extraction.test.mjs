import test from 'node:test';
import assert from 'node:assert/strict';
import { extractionSummary, destinationParts } from '../src/lib/extraction.ts';
const entry=(name,action='extract',extra={})=>({entry_id:name,source_name:name,output_name:name,size:10,is_dir:false,conflict:false,action,...extra});
const plan=entries=>({destination:'/out/project',entries,total_bytes:20});
test('outcome counts files independently from directory records',()=>{
 const result=extractionSummary(plan([entry('folder','extract',{is_dir:true}),entry('a'),entry('b','skip',{conflict:true})]));
 assert.equal(result.button,'Extract 1 file');assert.equal(result.outcome,'1 file to extract · 1 will be skipped');assert.equal(result.writing,2);
});
test('all skipped, empty, and blocked plans cannot yield extractable work',()=>{
 assert.equal(extractionSummary(plan([entry('a','skip')])).writing,0);
 assert.equal(extractionSummary(plan([entry('a','skip')])).outcome,'0 files to extract · 1 will be skipped');
 assert.equal(extractionSummary(null).button,'Nothing to extract');
 const result=extractionSummary(plan([entry('../a','blocked',{conflict:true})]));
 assert.equal(result.blocked.length,1);assert.equal(result.conflicts.length,1);assert.equal(result.writing,0);
});
test('keep-both and replace preserve conflict visibility and folder-only work remains possible',()=>{
 const result=extractionSummary(plan([entry('a','extract',{conflict:true,output_name:'a (2)'})]));
 assert.equal(result.conflicts.length,1);assert.equal(result.button,'Extract 1 file');
 assert.equal(extractionSummary(plan([entry('empty','extract',{is_dir:true})])).button,'Extract 1 folder');
});
test('destination labels support Windows, POSIX, roots and trailing separators',()=>{
 assert.equal(destinationParts('C:\\Users\\me\\Downloads\\project\\').name,'project');
 assert.equal(destinationParts('/Users/me/Downloads/project/').parent,'Downloads');
 assert.equal(destinationParts('/').name,'/');assert.equal(destinationParts('C:\\').name,'C:');
});
