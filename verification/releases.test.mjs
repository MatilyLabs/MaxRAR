import test from 'node:test';
import assert from 'node:assert/strict';
import {previewRelease,shouldAutoCheck,updateBusy,updateTitle} from '../src/lib/release.ts';
import {shortcutAction,commandShortcuts} from '../src/lib/shortcuts.ts';
test('MaxRAR help links share the configured repository, not the reference app',()=>{
 assert.equal(previewRelease.links.repository,'https://github.com/MatilyLabs/MaxRAR');
 assert.equal(previewRelease.links.manual,previewRelease.links.repository+'#user-manual');
 assert.equal(previewRelease.links.issues,previewRelease.links.repository+'/issues/new/choose');
 assert.equal(previewRelease.links.organization,'https://www.matily.org');
 assert.equal(previewRelease.configured,false);
});
test('automatic update checks respect configuration, preference and daily cadence',()=>{
 const now=200_000_000;
 assert.equal(shouldAutoCheck(true,true,0,now),true);
 assert.equal(shouldAutoCheck(false,true,0,now),false);
 assert.equal(shouldAutoCheck(true,false,0,now),false);
 assert.equal(shouldAutoCheck(true,true,now-86_400_000,now),true);
 assert.equal(shouldAutoCheck(true,true,now-1000,now),false);
 assert.equal(shouldAutoCheck(true,true,NaN,now),true);
 assert.equal(shouldAutoCheck(true,true,now+1,now),true);
});
test('unconfigured, offline and incomplete states never say up to date',()=>{
 for(const stage of ['idle','unavailable','error','checking','downloading','verifying'])assert.notEqual(updateTitle({stage}),'You’re up to date');
 assert.equal(updateTitle({stage:'current'}),'You’re up to date');
 assert.match(updateTitle({stage:'unavailable'},'package_manager'),/package manager/);
 assert.equal(updateBusy('verifying'),true);assert.equal(updateBusy('ready'),false);
});
test('shortcut list and keyboard dispatch use the same definitions',()=>{
 for(const s of commandShortcuts){
  assert.equal(shortcutAction({key:s.key,ctrlKey:true,metaKey:false,altKey:false,shiftKey:false}),s.id);
  assert.equal(shortcutAction({key:s.key,ctrlKey:false,metaKey:true,altKey:false,shiftKey:false}),s.id);
 }
 assert.equal(shortcutAction({key:'a',ctrlKey:false,metaKey:false,altKey:false,shiftKey:false}),undefined);
 assert.equal(shortcutAction({key:'o',ctrlKey:true,metaKey:false,altKey:true,shiftKey:false}),undefined);
});
