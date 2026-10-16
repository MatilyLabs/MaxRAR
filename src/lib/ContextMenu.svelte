<script lang="ts">
  import type {ContextAction} from './context';
  let {x,y,items,onchoose,ondismiss}:{x:number;y:number;items:ContextAction[];onchoose:(id:string)=>void;ondismiss:()=>void}=$props();
  let popup=$state<HTMLDivElement>();
  let left=$state(0),top=$state(0);
  const buttons=()=>popup?Array.from(popup.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')):[];
  $effect(()=>{items;if(!popup)return;const r=popup.getBoundingClientRect();left=Math.max(8,Math.min(x,window.innerWidth-r.width-8));top=Math.max(8,Math.min(y,window.innerHeight-r.height-8));buttons()[0]?.focus();});
  function key(event:KeyboardEvent){
    const list=buttons(),index=list.indexOf(document.activeElement as HTMLButtonElement);
    if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();event.stopPropagation();list[event.key==='Home'?0:event.key==='End'?list.length-1:(index+(event.key==='ArrowDown'?1:list.length-1))%list.length]?.focus();}
    else if(event.key==='Escape'||event.key==='Tab'){event.preventDefault();event.stopPropagation();ondismiss();}
    else if(event.key.length===1&&!event.ctrlKey&&!event.metaKey){const next=[...list.slice(index+1),...list.slice(0,index+1)].find(b=>b.textContent?.trim().toLowerCase().startsWith(event.key.toLowerCase()));if(next){event.preventDefault();next.focus();}}
  }
</script>
<svelte:window onpointerdown={event=>{if(!(event.target as HTMLElement).closest('.archive-context-menu'))ondismiss();}} onresize={ondismiss}/>
<div bind:this={popup} class="archive-context-menu" role="menu" aria-label="Archive actions" tabindex="-1" style:left={left+'px'} style:top={top+'px'} onkeydown={key} oncontextmenu={event=>event.preventDefault()}>
  {#each items as item}{#if item.separator}<div class="separator" role="separator"></div>{/if}<button role="menuitem" tabindex="-1" disabled={item.disabled} title={item.reason} class:danger={item.danger} onclick={()=>onchoose(item.id)}><img src={'/icons/'+item.icon+'.svg'} alt=""/><span>{item.label}</span></button>{/each}
</div>
<style>
  .archive-context-menu button.danger{background:transparent;border:0;color:var(--muted);font-weight:400}
  .archive-context-menu{position:fixed;z-index:80;width:max-content;min-width:225px;max-width:min(320px,calc(100vw - 16px));max-height:calc(100dvh - 16px);overflow:auto;padding:5px;background:var(--surface);border:1px solid var(--line);border-radius:7px;box-shadow:0 7px 24px #0003;user-select:none;outline:none}.archive-context-menu button{display:flex;align-items:center;gap:10px;width:100%;padding:8px 10px;text-align:left;font-size:12px;line-height:1.3;border-radius:4px;min-height:32px}.archive-context-menu img{width:17px;height:17px;flex-shrink:0}.archive-context-menu button span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.archive-context-menu button:hover:not(:disabled),.archive-context-menu button:focus-visible{background:var(--selection);outline:none}.archive-context-menu button.danger:not(:disabled){color:#b43832}.archive-context-menu button:disabled{opacity:.45}.separator{height:1px;background:var(--line);margin:5px 7px}:global(.application[data-theme=dark]) .archive-context-menu button.danger:not(:disabled){color:#ef9f97}@media(prefers-color-scheme:dark){:global(.application[data-theme=system]) .archive-context-menu button.danger:not(:disabled){color:#ef9f97}}
</style>
