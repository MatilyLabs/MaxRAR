import test from 'node:test';
import assert from 'node:assert/strict';
import { archiveOverview } from '../src/lib/overview.ts';
const entries=names=>names.map((name,id)=>({id:String(id),name,size:10,compressed_size:5,is_dir:name.endsWith('/'),is_encrypted:false}));
test('website inventory drives exact visual counts and a previewable starting file',()=>{
  const result=archiveOverview(entries(['site/index.html','site/pages/about.html','site/styles/main.css','site/scripts/app.js','site/images/logo.png']));
  assert.equal(result.files,5);assert.equal(result.folders,5);
  assert.equal(result.main.name,'site/index.html');assert.match(result.summary,/website/);
  assert.deepEqual(result.groups.map(g=>[g.id,g.count]),[['html',2],['css',1],['js',1],['images',1]]);
  assert.equal(result.folderRows[0].count,5);
});
test('paper classification does not expose raw TeX titles or assume every PDF is a figure',()=>{
  const result=archiveOverview(entries(['paper/main.tex','paper/references.bib','paper/figure.pdf','paper/figure.png']));
  assert.match(result.summary,/Likely research sources/);assert.equal(result.folders,1);
  assert.equal(result.groups.find(g=>g.id==='images').count,2);
  assert.equal(result.groups.find(g=>g.id==='images').label,'Possible figures');
  assert.equal(archiveOverview(entries(['report.pdf'])).groups[0].id,'documents');
});
test('bounded source hints preserve extensionless paper recognition',()=>{
  const list=entries(['download']);
  assert.equal(archiveOverview(list).main,null);
  const result=archiveOverview(list,['0']);
  assert.equal(result.groups[0].id,'tex');assert.equal(result.main.id,'0');
  assert.match(result.basis,/source hints/);
});
test('mixed inventories keep total counts when small categories are combined',()=>{
  const list=entries(['a.html','b.css','c.js','d.png','e.md','f.rs','g.csv','h.zip','i.mp4','unknown']);
  const result=archiveOverview(list);
  assert.equal(result.groups.length,5);assert.equal(result.groups.reduce((n,g)=>n+g.count,0),list.length);
  assert.equal(new Set(result.groups.map(g=>g.id)).size,result.groups.length);
});
test('empty archives, implicit folders and unknown content stay conservative',()=>{
  assert.equal(archiveOverview([]).summary,'This archive is empty.');
  assert.equal(archiveOverview(entries(['empty/'])).files,0);
  assert.equal(archiveOverview(entries(['root/child/'])).folderRows[0].description,'No files listed');
  assert.equal(archiveOverview(entries(['Cargo.toml'])).summary,'Contains project configuration files.');
  const unknown=archiveOverview(entries(['scripts/launch.sh','random.payload']));
  assert.equal(unknown.main,null);assert.doesNotMatch(unknown.summary,/safe|website|research/);
  assert.equal(unknown.folderRows[0].description,'source code');
});
test('main candidates prefer the archive context and shallow paths deterministically',()=>{
  const result=archiveOverview(entries(['deep/example/index.html','README.md','index.html','setup.sh']));
  assert.equal(result.main.name,'index.html');
  assert.equal(archiveOverview(entries(['project/README.md','project/main.rs'])).main.name,'project/README.md');
});
