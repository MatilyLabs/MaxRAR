<script lang="ts">
 import {onMount} from 'svelte';
 import {command,isDesktop} from './bridge';
 let status=$state<{platform:string;available:boolean;installed:boolean;message:string}|null>(null),working=$state(false),message=$state('');
 onMount(()=>{if(isDesktop())void command<typeof status>('shell_integration_status').then(s=>status=s).catch(e=>message=String(e));});
 async function enable(){working=true;message='';try{message=await command<string>('install_shell_integration');status=await command<typeof status>('shell_integration_status');}catch(e){message=String(e);}finally{working=false;}}
</script>
<section class="shell-settings"><img src="/icons/folder.svg" alt=""/><h3>Work directly from your desktop</h3><p>Right-click a file or folder to compress it, or an archive to review extraction in MaxRAR.</p><div class="integration-state"><strong>{status?.installed?'Desktop actions enabled':status?'Desktop actions not enabled':'Checking desktop integration…'}</strong><p>{status?.message||'Open the desktop app to manage integration.'}</p></div>{#if message}<p role="status">{message}</p>{/if}<button class="primary" disabled={working||!status?.available} onclick={enable}>{working?'Enabling…':status?.installed?'Refresh desktop actions':'Enable desktop actions'}</button></section>
<style>.shell-settings{text-align:center;font-size:13px;line-height:1.6}.shell-settings>img{width:44px;height:44px}.shell-settings h3{font-size:18px;margin:10px 0}.shell-settings p{color:var(--muted);margin:8px 0}.integration-state{border:1px solid var(--line);padding:14px;margin:20px 0;border-radius:6px;background:var(--soft2);text-align:left}.integration-state strong{font-size:13px}.integration-state p{font-size:12px}.shell-settings>button{margin-top:10px}</style>
