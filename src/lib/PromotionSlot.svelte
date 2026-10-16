<script lang="ts">
 import {onMount} from 'svelte';
 import {command,isDesktop} from './bridge';
 import PromotionCard from './PromotionCard.svelte';
 import type {PromotionFeed} from './promotions';
 let feed=$state<PromotionFeed|null>(null);
 onMount(()=>{
  if(!isDesktop())return;
  let disposed=false;
  async function refresh(){try{const next=await command<PromotionFeed|null>('promotion_feed');if(!disposed)feed=next;}catch{if(!disposed)feed=null;}}
  void refresh();const timer=setInterval(()=>void refresh(),60*60*1000);
  return()=>{disposed=true;clearInterval(timer);};
 });
</script>
<PromotionCard {feed} onopen={async id=>{await command('open_promotion',{campaignId:id});}}/>
