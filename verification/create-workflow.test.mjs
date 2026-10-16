import test from 'node:test';
import assert from 'node:assert/strict';
import { createFormats,createProblem,replaceArchiveSuffix,suggestedArchivePath } from '../src/lib/create.ts';
const files=[{path:'/docs/report.pdf',type:'file'},{path:'/docs/notes.txt',type:'file'}];
test('chosen format and actual encoder suffix must agree',()=>{
  assert.equal(createProblem(files,'zip','/out/archive.zip',false,'',''),'');
  assert.notEqual(createProblem(files.slice(0,1),'gz','/out/archive.tar.gz',false,'',''),'');
  assert.notEqual(createProblem(files,'zip','/out/archive.7z',false,'',''),'');
  assert.equal(replaceArchiveSuffix('/out/backup.tar.gz','7z'),'/out/backup.7z');
});
test('single-file formats reject multiple sources and folders without discarding them',()=>{
  assert.notEqual(createProblem(files,'gz','/out/archive.gz',false,'',''),'');
  assert.notEqual(createProblem([{path:'/docs',type:'folder'}],'xz','/out/docs.xz',false,'',''),'');
  assert.equal(createProblem(files,'tar.gz','/out/archive.tar.gz',false,'',''),'');
});
test('password creation requires a matching confirmation on a supported format',()=>{
  assert.notEqual(createProblem(files,'zip','/out/a.zip',true,'',''),'');
  assert.notEqual(createProblem(files,'zip','/out/a.zip',true,'secret','different'),'');
  assert.equal(createProblem(files,'zip','/out/a.zip',true,'secret','secret'),'');
  assert.notEqual(createProblem(files,'tar','/out/a.tar',true,'secret','secret'),'');
});
test('suggested paths preserve raw-stream filenames and reject original-file destinations',()=>{
  assert.equal(suggestedArchivePath(files.slice(0,1),'gz'),'/docs/report.pdf.gz');
  assert.equal(suggestedArchivePath(files.slice(0,1),'zip'),'/docs/report.zip');
  assert.equal(suggestedArchivePath([{path:'/docs/source.zip',type:'file'}],'zip'),'/docs/source archive.zip');
  assert.equal(suggestedArchivePath([{path:'C:\\Docs\\Project',type:'folder'}],'zip'),'C:\\Docs\\Project.zip');
  assert.notEqual(createProblem([{path:'/docs/source.zip',type:'file'}],'zip','/docs/source.zip',false,'',''),'');
});

test('MRAR is an explicit experimental option and ZIP remains the default',()=>{
  assert.equal(createFormats[0].id,'zip');
  const mrar=createFormats.find(format=>format.id==='mrar');
  assert.ok(mrar);assert.match(mrar.label,/experimental/i);
  assert.match(mrar.hint,/shared compression.*small files/i);
  assert.equal(mrar.password,false);assert.equal(mrar.single,false);
});
test('MRAR creation accepts multiple files or folders and requires its own suffix',()=>{
  assert.equal(createProblem(files,'mrar','/out/archive.mrar',false,'',''),'');
  assert.equal(createProblem([{path:'/docs/project',type:'folder'}],'mrar','/out/project.mrar',false,'',''),'');
  assert.notEqual(createProblem(files,'mrar','/out/archive.zip',false,'',''),'');
  assert.notEqual(createProblem(files,'zip','/out/archive.mrar',false,'',''),'');
  assert.equal(replaceArchiveSuffix('/out/archive.tar.gz','mrar'),'/out/archive.mrar');
  assert.equal(replaceArchiveSuffix('/out/archive.mrar','zip'),'/out/archive.zip');
  assert.equal(suggestedArchivePath(files.slice(0,1),'mrar'),'/docs/report.mrar');
  assert.notEqual(createProblem(files,'mrar','/out/archive.mrar',true,'secret','secret'),'');
});
