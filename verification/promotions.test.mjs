import test from 'node:test';
import assert from 'node:assert/strict';
import {activeCampaigns,readDismissed} from '../src/lib/promotions.ts';
const campaign={id:'launch',title:'Product',description:'Discover tools',button:'Learn more',url:'https://github.com/MatilyLabs',icon:'app',startsAt:null,endsAt:null};
const feed={schemaVersion:1,enabled:true,validUntil:200,rotationSeconds:10,campaigns:[campaign]};
test('disabled, expired and absent campaigns leave no promotion',()=>{
 assert.deepEqual(activeCampaigns(null,[],100),[]);
 assert.deepEqual(activeCampaigns({...feed,enabled:false},[],100),[]);
 assert.deepEqual(activeCampaigns(feed,[],200),[]);
});
test('campaign schedules and dismissal are respected at boundaries',()=>{
 const scheduled={...feed,campaigns:[{...campaign,startsAt:110,endsAt:150}]};
 assert.equal(activeCampaigns(scheduled,[],109).length,0);
 assert.equal(activeCampaigns(scheduled,[],110).length,1);
 assert.equal(activeCampaigns(scheduled,[],150).length,0);
 assert.equal(activeCampaigns(feed,['launch'],100).length,0);
});
test('corrupt dismissal storage does not break the inspector',()=>{
 assert.deepEqual(readDismissed('invalid'),[]);
 assert.deepEqual(readDismissed('{}'),[]);
 assert.deepEqual(readDismissed('["launch",3,null]'),['launch']);
});
