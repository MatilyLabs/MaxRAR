<script lang="ts">
 import {onMount} from 'svelte';
 import appIcon from '../../design/app-icon.svg?url';
 import {activeCampaigns,readDismissed} from './promotions';
 import type {PromotionFeed} from './promotions';
 let {feed=null,onopen}:{feed?:PromotionFeed|null;onopen:(id:string)=>Promise<void>}=$props();
 let dismissed=$state<string[]>([]),now=$state(Math.floor(Date.now()/1000)),index=$state(0),hovered=$state(false),focused=$state(false),manualPause=$state(false),reducedMotion=$state(false),failure=$state(''),opening=$state(false);
 const campaigns=$derived(activeCampaigns(feed,dismissed,now));
 const current=$derived(campaigns[index%Math.max(1,campaigns.length)]);
 const storage='maxrar.promotions.dismissed';
 onMount(()=>{
  try{dismissed=readDismissed(localStorage.getItem(storage));}catch{}
  const media=matchMedia('(prefers-reduced-motion: reduce)');reducedMotion=media.matches;
  const motion=()=>reducedMotion=media.matches;media.addEventListener('change',motion);
  return()=>media.removeEventListener('change',motion);
 });
 $effect(()=>{
  if(!feed?.enabled)return;
  now=Math.floor(Date.now()/1000);
  const clock=setInterval(()=>now=Math.floor(Date.now()/1000),1000);
  return()=>clearInterval(clock);
 });
 $effect(()=>{
  if(!feed?.enabled||feed.campaigns.length<2)return;
  const seconds=Math.max(10,feed?.rotationSeconds||10);
  const timer=setInterval(()=>{if(campaigns.length>1&&!hovered&&!focused&&!manualPause&&!reducedMotion&&document.visibilityState==='visible'){index=(index+1)%campaigns.length;failure='';}},seconds*1000);
  return()=>clearInterval(timer);
 });
 function dismiss(){if(!current)return;dismissed=[...dismissed,current.id].slice(-100);index=0;failure='';try{localStorage.setItem(storage,JSON.stringify(dismissed));}catch{}}
 async function open(){if(!current||opening)return;opening=true;failure='';try{await onopen(current.id);}catch{failure='Link unavailable. Please try later.';}finally{opening=false;}}
</script>
{#if current}
 <section class="promotion" aria-label="From Matily" aria-roledescription="carousel" onmouseenter={()=>hovered=true} onmouseleave={()=>hovered=false} onfocusin={()=>focused=true} onfocusout={e=>{if(!e.currentTarget.contains(e.relatedTarget as Node|null))focused=false;}}>
  <header><span>From Matily</span><button class="dismiss" aria-label="Dismiss this promotion" onclick={dismiss}>×</button></header>
  <div class="product" class:slide={!reducedMotion}>{#key current.id}<img src={current.icon==='app'?appIcon:'/icons/'+current.icon+'.svg'} alt=""/><div><h3>{current.title}</h3><p>{current.description}</p></div>{/key}</div>
  <div class="actions"><button class="learn" disabled={opening} onclick={open}>{current.button}<span aria-hidden="true">↗</span></button>{#if campaigns.length>1}<div class="promo-navigation" aria-label="Promotions">{#each campaigns as campaign,i}<button class:active={current.id===campaign.id} aria-label={'Show promotion '+(i+1)} aria-pressed={current.id===campaign.id} onclick={()=>{index=i;failure='';}}><span></span></button>{/each}<button class="pause" aria-label={manualPause?'Resume promotion rotation':'Pause promotion rotation'} aria-pressed={manualPause} onclick={()=>manualPause=!manualPause}>{manualPause?'▶':'Ⅱ'}</button></div>{/if}</div>
  {#if failure}<p class="failure" role="status">{failure}</p>{/if}
 </section>
{/if}
<style>
 .promotion{flex-shrink:0;border-top:1px solid var(--line);margin:0 0 12px;padding:12px 0 0;font-size:12px;line-height:1.5;color:var(--text)}
 header{display:flex;align-items:center;justify-content:space-between;color:var(--muted);font-size:10px;margin-bottom:8px}button{border-radius:4px}.dismiss{height:24px;width:24px;font-size:18px;color:var(--muted);padding:0}.dismiss:hover,.promo-navigation button:hover{background:var(--soft)}
 .product{display:flex;gap:10px;align-items:flex-start;min-height:74px}.product>img{width:32px;height:32px;flex-shrink:0;margin-top:3px}.product>div{min-width:0}h3{font-size:13px;font-weight:600;margin:0 0 3px;overflow-wrap:anywhere}p{margin:0;color:var(--muted);font-size:11px;overflow-wrap:anywhere}.actions{display:flex;align-items:center;justify-content:space-between;gap:6px;margin-top:8px;flex-wrap:wrap}.learn{display:inline-flex;align-items:center;gap:7px;color:var(--accent);border:1px solid var(--line);padding:5px 9px;background:var(--surface);font-size:11px;min-height:30px}.learn:hover{background:var(--soft)}.promo-navigation{display:flex;align-items:center;gap:0}.promo-navigation button{width:20px;height:24px;display:grid;place-items:center;padding:0}.promo-navigation button>span{height:5px;width:5px;background:var(--muted);opacity:.4;border-radius:50%}.promo-navigation button.active>span{background:var(--accent);opacity:1}.promo-navigation .pause{font-size:10px;color:var(--muted);margin-left:3px}.failure{margin-top:8px;font-size:11px}.slide>div{animation:enter .18s ease-out}@keyframes enter{from{opacity:.2;transform:translateX(5px)}to{opacity:1;transform:translateX(0)}}
</style>
