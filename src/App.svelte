<script lang="ts">
  import { onMount, tick } from 'svelte';
  import appIcon from '../design/app-icon.svg?url';
  import ScanPanel from './lib/ScanPanel.svelte';
  import CreatePanel from './lib/CreatePanel.svelte';
  import ShellIntegration from './lib/ShellIntegration.svelte';
  import type {CreateSource} from './lib/create';
  let createSources=$state<CreateSource[]>([]);
  let shellExtractPath='';
  import ExtractPanel from './lib/ExtractPanel.svelte';
  import EncryptedBadge from './lib/EncryptedBadge.svelte';
  import PasswordPanel from './lib/PasswordPanel.svelte';
  import TestPanel from './lib/TestPanel.svelte';
  import AboutPanel from './lib/AboutPanel.svelte';
  import PromotionSlot from './lib/PromotionSlot.svelte';
  import ShortcutsPanel from './lib/ShortcutsPanel.svelte';
  import {shortcutAction} from './lib/shortcuts';
  import {previewRelease,shouldAutoCheck,updateBusy} from './lib/release';
  import type {ReleaseInfo,UpdateView,ProjectLink} from './lib/release';
  import type { TestReceipt } from './lib/integrity';
  import ArchiveOverview from './lib/ArchiveOverview.svelte';
  import ContextMenu from './lib/ContextMenu.svelte';
  import {contextActions,directChild,scanEntryIds,selectedArchiveIds} from './lib/context';
  import type { CreateDraft } from './lib/create';
  import type { ScanReport } from './lib/scan';
  import { scanOutcome } from './lib/scan';
  import { listen } from '@tauri-apps/api/event';
  import { command, chooseArchive, chooseDestination, bytes, kind, iconFor, isDesktop, shellQuote, formatModified } from './lib/bridge';
  import type { ArchiveInfo, Entry, Job } from './lib/bridge';
  let archive = $state<ArchiveInfo | null>(null), selected = $state<string[]>([]), query = $state(''), folder = $state('');
  let inspector = $state(true), menu = $state(''), dialog = $state(''), busy = $state(false), error = $state(''), notice = $state('');
  let jobs = $state<Job[]>([]), theme = $state('system'), compact = $state(false), sortKey = $state('name'), ascending = $state(true), dateFormat = $state('locale');
  let searchInput: HTMLInputElement; let dialogEl = $state<HTMLDialogElement>(); let lastFocus: HTMLElement | null = null;
  let destination = $state(''), policy = $state('skip'), extractionPlan = $state<any>(null), preview = $state(''), previewTitle = $state(''), comment = $state('');
  let model = $state(''), password = $state(''), pendingAction = $state(''), favorites = $state<string[]>([]), sample = $state(false);
  let rowScroll = $state(0), viewportHeight = $state(480); let fileScroller = $state<HTMLDivElement>();
  $effect(()=>{folder;query;if(fileScroller)fileScroller.scrollTop=0;rowScroll=0;});
  let scannerReport=$state<ScanReport|null>(null), scanningJob=$state(''), planning=$state(false), planError=$state(''), extractionReview=$state('');
  let testJob=$state(''),testReceipt=$state<TestReceipt|null>(null),testError=$state(''),testNeedsPassword=$state(false),testName=$state(''),testFiles=$state(0),testFormat=$state(''),testEncrypted=$state(false);
  let releaseInfo=$state<ReleaseInfo>(previewRelease),updateView=$state<UpdateView>({stage:'idle'}),autoUpdates=$state(true);
  let lastUpdateAttempt=0;
  let planGeneration=0;
  let openAfter=$state(true);
  let planContext='';
  let contextMenu=$state<{x:number;y:number;background:boolean}|null>(null),infoEntries=$state<Entry[]>([]),infoMembers=$state<Entry[]>([]);
  let contextOrigin:HTMLElement|null=null;
  let scanSelection=$state<string[]|null>(null),selectionScanReport=$state<ScanReport|null>(null),scanSubject=$state('');
  const activeScanReport=$derived(scanSelection===null?scannerReport:selectionScanReport);
  interface ResearchProfile {kind:string;archive_identity:string;title:string|null;main_candidates:{entry_id:string;path:string;line_start:number}[];bibliographies:{entry_id:string;path:string}[];figures:{entry_id:string;path:string}[];inspected_tex_files:number;tex_files:number;limited:boolean;warnings:string[];}
  let researchProfile=$state<ResearchProfile|null>(null), paperLoading=$state(false), profileError=$state('');let profileGeneration=0;
  $effect(()=>{password;scannerReport=null;selectionScanReport=null;researchProfile=null;});
  $effect(()=>{const state={path:archive?.path,archive_identity:archive?.identity,destination,entry_ids:selectedIds(),policy,password};const expectedIdentity=archive?.identity;const generation=++planGeneration;if(dialog!=='extract'||!state.path||!destination.trim()||sample||!isDesktop()){planning=false;return;}const context=JSON.stringify([state.path,state.archive_identity,destination,state.entry_ids,password]);if(context!==planContext){extractionPlan=null;planContext=context;}planError='';extractionReview='';planning=true;const timer=setTimeout(async()=>{try{const result=await command<any>('plan_extraction',{request:state});if(generation===planGeneration){if(expectedIdentity&&result.archive_identity!==expectedIdentity){planError='The archive changed on disk. Close this sheet and reopen the archive before extracting.';scannerReport=null;}else extractionPlan=result;}}catch(e){if(generation===planGeneration)planError=String(e).replace(/^Error:\s*/,'');}finally{if(generation===planGeneration)planning=false;}},300);return()=>{clearTimeout(timer);planGeneration++;};});
  const menuNames = ['File','Commands','Tools','Favorites','Options','Help'];
  const actions = [{id:'add',label:'Add',hint:'Create a new archive'}, {id:'extract',label:'Extract To',hint:'Review destination and conflicts'}, {id:'test',label:'Test',hint:'Check archive integrity'}, {id:'view',label:'View',hint:'Preview a selected file'}, {id:'delete',label:'Delete',hint:'Delete selected entries where supported'}, {id:'find',label:'Find',hint:'Find filenames and paths'}, {id:'wizard',label:'Wizard',hint:'Guided archive tasks'}, {id:'info',label:'Info',hint:'Archive information and capabilities'}, {id:'scan',label:'Scan',hint:'Run built-in local inspection'}, {id:'comment',label:'Comment',hint:'Read or update the archive comment'}];
  const allEntries = $derived(archive?.entries || []);
  const extractSource=$derived.by(()=>{const ids=new Set(selectedIds());const entries=extractionPlan?.entries||allEntries.filter(e=>!ids.size||ids.has(e.id));const files=entries.filter((e:any)=>!e.is_dir);return {files:files.length,bytes:files.reduce((sum:number,e:any)=>sum+e.size,0)};});
  const folders = $derived([...new Set(allEntries.flatMap(e => { const parts=e.name.replace(/\/$/,'').split('/'); return parts.slice(0,e.is_dir ? parts.length : -1).map((_,i)=>parts.slice(0,i+1).join('/')); }))].sort());
  const knownPaths=$derived(new Set(allEntries.map(e=>e.name.replace(/\/$/,''))));
  const selectedSet=$derived(new Set(selected));
  const browsingEntries = $derived<Entry[]>([...allEntries, ...folders.filter(path=>!knownPaths.has(path)).map(path=>({id:'folder:'+path,name:path+'/',size:0,compressed_size:0,is_dir:true,is_encrypted:false}))]);
  const rows = $derived.by(() => {
    let list = browsingEntries.filter(e => query ? e.name.toLowerCase().includes(query.toLowerCase()) : directChild(e,folder));
    return list.sort((a,b)=> { if(a.is_dir!==b.is_dir) return a.is_dir?-1:1; const cmp=sortKey==='size'?a.size-b.size:sortKey==='type'?kind(a).localeCompare(kind(b)):sortKey==='modified'?(a.modified||'').localeCompare(b.modified||''):a.name.localeCompare(b.name,undefined,{numeric:true}); return ascending?cmp:-cmp; });
  });
  const rowHeight = $derived(compact?30:39), start = $derived(Math.max(0,Math.floor(rowScroll/rowHeight)-5)), end = $derived(Math.min(rows.length,start+Math.ceil(viewportHeight/rowHeight)+10)), visibleRows = $derived(rows.slice(start,end));
  const selectedEntries = $derived(browsingEntries.filter(e=>selectedSet.has(e.id)));
  const contextItems=$derived(contextActions(contextMenu?.background?[]:selectedEntries,{archive:!!archive,canExtract:capability('extract')&&!sample,canDelete:capability('delete'),canScan:isDesktop()&&!sample&&!!archive&&(contextMenu?.background||scanEntryIds(allEntries,selectedEntries).length>0),canCreate:isDesktop(),busy}));
  const activeJobs = $derived(jobs.filter(j=>['running','queued','pending'].includes(j.status)));
  const createJob = $derived([...jobs].reverse().find(j=>['create','Create archive'].includes(j.kind)&&['running','queued'].includes(j.status)));
  function disabled(id:string) { return busy || (id==='add' ? !isDesktop() : id==='find' ? !archive : id==='scan'?(!isDesktop()||sample):id==='wizard'?false : !archive || (!sample&&((id==='extract'&&!capability('extract'))||(id==='test'&&!capability('test')))) || (id==='view'&&(selectedEntries.length!==1||selectedEntries[0].is_dir)) || (id==='delete'&&(!selected.length||!capability('delete')))); }
  function capability(key:string) { return !!archive?.capabilities?.['can_'+key]; }
  function selectedIds(){if(!selected.length)return [];const ids=selectedArchiveIds(allEntries,selectedEntries);return ids.length?ids:selected;}
  async function job<T>(kind:string,operation:string,args:Record<string,unknown>,id=crypto.randomUUID()){jobs=[...jobs,{id,kind,status:'queued',progress:0,message:'Waiting for worker'}];try{const result=await command<T>(operation,{...args,jobId:id});jobs=jobs.map(j=>j.id===id?{...j,status:'completed',progress:1,receipt:result}:j);return result;}catch(e){jobs=jobs.map(j=>j.id===id?{...j,status:/cancel/i.test(String(e))?'cancelled':'failed',message:String(e)}:j);throw e;}}
  function persist() { localStorage.setItem('maxrar.workspace',JSON.stringify({theme,compact,sortKey,ascending,dateFormat,model,favorites,inspector})); }
  function sort(key:string) { ascending=sortKey===key?!ascending:true;sortKey=key;persist(); }
  async function show(name:string) { const focused=document.activeElement as HTMLElement;lastFocus=(focused?.closest('.menu-root')?.querySelector(':scope > button') as HTMLElement)||focused;contextMenu=null;menu='';dialog=name; await tick(); dialogEl?.showModal();(dialogEl?.querySelector(name==='extract'?'.destination-row button':name==='about'?'.update-actions button:not(:disabled)':'input,textarea,select,button.primary') as HTMLElement)?.focus(); }
  function closeDialog() { if(dialog==='password')shellExtractPath='';dialogEl?.close(); dialog=''; lastFocus?.focus(); }
  async function run<T>(label:string, fn:()=>Promise<T>):Promise<T|undefined> { busy=true; error=''; try{return await fn();} catch(e){error=String(e).replace(/^Error:\s*/,'');} finally{busy=false;} }
  async function openPath(path:string,initialPassword?:string) { if(shellExtractPath&&shellExtractPath!==path)shellExtractPath='';if(initialPassword!==undefined)password=initialPassword;else if(archive?.path!==path&&pendingAction!=='open:'+path)password='';const result=await run('Opening',()=>command<ArchiveInfo>('inspect_archive',{path,password:password||null})); if(result){archive=result;selected=[];query='';folder='';sample=false;scannerReport=null;selectionScanReport=null;scanSelection=null;researchProfile=null;contextMenu=null;pendingAction='';notice='Archive inventory loaded. Contents have not been executed.';void loadResearchProfile(result);if(shellExtractPath===path){shellExtractPath='';await action('extract');}}else if(/password|decrypt|encrypted/i.test(error)){pendingAction='open:'+path;password='';await show('password');}else if(shellExtractPath===path)shellExtractPath=''; }
  async function initializeShellRequest(){
    try{const request=await command<{kind:'compress'|'extract';sources:CreateSource[]}|null>('initial_shell_request');
      if(request?.kind==='compress'){createSources=request.sources;password='';await show('create');}
      else if(request?.kind==='extract'){shellExtractPath=request.sources[0].path;await openPath(shellExtractPath);}
      else{const path=await command<string|null>('initial_archive_path');if(path)await openPath(path);}
    }catch(e){error=String(e);}
  }
  async function openArchive() { menu='';const path=await run('Choose archive',chooseArchive);if(path)await openPath(path); }
  function select(e:Entry,event:MouseEvent) { selected=event.metaKey||event.ctrlKey ? selected.includes(e.id)?selected.filter(x=>x!==e.id):[...selected,e.id]:[e.id]; }
  function enter(e:Entry) { if(e.is_dir){folder=e.name.replace(/\/$/,'');query='';selected=[];rowScroll=0;}else void viewEntry(e); }
  function continuePassword(secret:string){
    password=secret;const shellPath=shellExtractPath;closeDialog();shellExtractPath=shellPath;
    if(pendingAction==='test')void performTest(true);
    else if(pendingAction.startsWith('open:'))void openPath(pendingAction.slice(5));
    else{const entry=allEntries.find(e=>e.id===pendingAction.slice(8));if(entry)void viewEntry(entry,true);}
  }
  async function viewEntry(e:Entry,confirmed=false) { if(e.is_encrypted&&!confirmed){pendingAction='preview:'+e.id;await show('password');return;}if(sample){previewTitle=e.name;preview='Sample workspace — preview content is not loaded. Open a real archive in the desktop app to inspect its contents.';await show('preview');return;} const r=await run('Preview',()=>command<any>('preview_entry',{path:archive!.path,entryId:e.id,password:password||null})); if(r){previewTitle=e.name;preview=typeof r==='string'?r:r.content||r.text||JSON.stringify(r,null,2); if(r.truncated)preview+='\n\n[Preview truncated at the configured limit]';await show('preview');}else if(/password|decrypt/i.test(error)){pendingAction='preview:'+e.id;password='';await show('password');} }
  async function action(id:string) { menu='';if(disabled(id))return; switch(id){case 'add':createSources=[];password='';await show('create');break;case 'extract':destination=suggestedDestination();policy='skip';openAfter=true;extractionPlan=null;await show('extract');break;case 'test':await performTest();break;case 'view':if(selectedEntries[0])await viewEntry(selectedEntries[0]);break;case 'delete':await show('delete');break;case 'find':searchInput?.focus();break;case 'wizard':await show('wizard');break;case 'info':await show('info');break;case 'scan':if(!archive)await openArchive();if(archive){scanSelection=null;scanSubject=archive.name;await show('scanner');void scanArchive();}break;case 'comment':comment=archive?.comment||'';await show('comment');break;} }
  async function performTest(confirmed=false){
    if(!archive||testJob)return;
    testName=archive.name;testFiles=allEntries.filter(e=>!e.is_dir).length;testFormat=archive.format;testEncrypted=allEntries.some(e=>e.is_encrypted);testReceipt=null;testError='';testNeedsPassword=testEncrypted&&!confirmed;
    await show('test');
    if(testNeedsPassword)return;
    if(sample||!isDesktop()){testError='Open a real archive in the desktop app to test its contents.';return;}
    const target={path:archive.path,password:password||null,archive_identity:archive.identity};
    const id=crypto.randomUUID();testJob=id;busy=true;error='';
    try{testReceipt=await job<TestReceipt>('Integrity test','test_archive',{request:target},id);notice='Archive integrity test completed.';}
    catch(e){testError=String(e).replace(/^Error:\s*/,'');notice='Archive test did not complete successfully.';}
    finally{testJob='';busy=false;}
  }
  function testPassword(){pendingAction='test';void show('password');}
  async function cancelTest(){if(!testJob)return;try{await command('cancel_job',{jobId:testJob});}catch(e){notice='Could not cancel test: '+String(e);}}
  async function createArchive(request:CreateDraft){const result=await run('Create archive',()=>job<ArchiveInfo>('Create archive','create_archive',{request}));if(!result)throw new Error(error||'The archive could not be created.');if(dialog==='create')closeDialog();// Creation does not unlock the archive for subsequent content access.
    await openPath(request.output,'');}
  async function chooseFolder(){const dest=await run('Choose destination',chooseDestination);if(dest){destination=dest;extractionPlan=null;}}
  function suggestedDestination(){if(!archive)return '';const cut=Math.max(archive.path.lastIndexOf('/'),archive.path.lastIndexOf('\\'));let stem=archive.name.replace(/\.(tar\.(gz|bz2|xz|zst|zstd|br)|tgz|tbz|tbz2|txz|tzst|zip|zipx|7z|mrar|rar|tar|gz|bz2|xz|zst|zstd|br)$/i,'')||'Extracted';if(stem===archive.name)stem=archive.name+' extracted';return archive.path.slice(0,cut+1)+stem;}
  function conflictKey(plan:any){return JSON.stringify((plan?.entries||[]).filter((e:any)=>e.conflict).map((e:any)=>[e.entry_id,e.output_name,e.action]));}
  async function extract(){if(!archive||!destination.trim()||busy||sample||planning||planError||!extractionPlan||(extractionPlan.requested_destination??extractionPlan.destination)!==destination)return;const shown=extractionPlan;const openWhenFinished=openAfter;const request={path:archive.path,archive_identity:archive.identity,destination,entry_ids:selectedIds(),policy,password:password||null};await run('Extracting',async()=>{const fresh=await command<any>('plan_extraction',{request});if(dialog!=='extract'||archive?.path!==request.path||destination!==request.destination||password!==(request.password||'')||policy!==request.policy||JSON.stringify(selectedIds())!==JSON.stringify(request.entry_ids)){extractionReview='Extraction choices changed. Review the current settings, then extract again.';return;}if(archive?.identity&&fresh.archive_identity!==archive.identity){scannerReport=null;extractionPlan=null;extractionReview='The archive changed on disk. Close this sheet and reopen the archive to review its current files.';return;}extractionPlan=fresh;planError='';if(fresh.entries.some((e:any)=>e.action==='blocked')){extractionReview='Some entries are blocked. Review the reasons below; nothing has been written.';return;}if(fresh.entries.some((e:any)=>e.conflict)&&(!shown||conflictKey(shown)!==conflictKey(fresh))){extractionReview='Destination conflicts changed. Review the updated choices, then select Extract again.';return;}closeDialog();const receipt=await job<any>('Extraction','extract_archive',{request:fresh,password:password||null,openAfter:openWhenFinished});notice=receipt.message||'Extraction finished. Review the retained task receipt.';});}
  async function deleteSelected(){closeDialog();await run('Delete entries',()=>job('Delete entries','delete_entries',{request:{path:archive!.path,archive_identity:archive!.identity,entry_ids:selectedIds()}}));if(!error)await openPath(archive!.path);}
  async function saveComment(){await run('Save comment',()=>job('Update comment','update_comment',{request:{path:archive!.path,archive_identity:archive!.identity,comment}}));if(!error){closeDialog();await openPath(archive!.path);}}
  async function scanArchive(){
    if(!archive||sample||scanningJob)return;
    const target={path:archive.path,identity:archive.identity,password,ids:scanSelection===null?null:[...scanSelection]};
    if(target.ids?.length===0){notice='Select at least one file to scan.';return;}
    if(target.ids===null)scannerReport=null;else selectionScanReport=null;
    const id=crypto.randomUUID();scanningJob=id;
    try{
      const report=await run('Built-in inspection',()=>job<ScanReport>(target.ids?'Scan selected files':'Scan archive','scan_archive',{path:target.path,password:target.password||null,entryIds:target.ids,expectedIdentity:target.identity},id));
      if(!report&&/archive changed|changed since review|changed during inspection/i.test(error)){scannerReport=null;selectionScanReport=null;notice='Archive changed on disk. Reopen it before scanning.';}
      const scopeMatches=report&&(target.ids===null?report.scope?.kind!=='selection':report.scope?.kind==='selection'&&JSON.stringify([...report.scope.entry_ids].sort())===JSON.stringify([...target.ids].sort()));
      if(report&&scopeMatches&&archive?.path===target.path&&archive?.identity===report.archive_identity&&password===target.password&&JSON.stringify(scanSelection)===JSON.stringify(target.ids)){
        if(target.ids===null)scannerReport=report;else selectionScanReport=report;
        notice=`${target.ids?'Selection scan':'Archive scan'} · ${scanOutcome(report).title} · ${report.coverage.scanned_files}/${report.coverage.total_files} files checked`;
      }else if(report)notice='Archive or scan selection changed. Run a new scan for the current files.';
    }finally{scanningJob='';}
  }
  function dismissContext(){contextMenu=null;if(contextOrigin?.isConnected)contextOrigin.focus();}
  function openContext(event:MouseEvent,entry?:Entry){
    event.preventDefault();event.stopPropagation();menu='';window.getSelection()?.removeAllRanges();
    contextOrigin=(event.currentTarget as HTMLElement).matches('button,[tabindex]')?event.currentTarget as HTMLElement:(fileScroller?.querySelector<HTMLElement>('[role=grid]')||document.activeElement as HTMLElement);
    if(entry&&!selected.includes(entry.id))selected=[entry.id];
    contextMenu={x:event.clientX,y:event.clientY,background:!entry};
  }
  function keyboardContext(event:KeyboardEvent,entry?:Entry){
    if(event.key!=='ContextMenu'&&!(event.shiftKey&&event.key==='F10'))return;
    event.preventDefault();event.stopPropagation();
    const r=(event.currentTarget as HTMLElement).getBoundingClientRect();
    openContext({preventDefault:()=>{},stopPropagation:()=>{},currentTarget:event.currentTarget,clientX:r.left+Math.min(120,r.width/2),clientY:r.bottom} as unknown as MouseEvent,entry);
  }
  async function copyContext(text:string){try{await navigator.clipboard.writeText(text);notice='Copied to clipboard.';}catch{previewTitle='Copy text';preview=text;await show('preview');}}
  async function contextAction(id:string){
    contextMenu=null;if(contextOrigin?.isConnected)contextOrigin.focus();
    switch(id){
      case 'openArchive':await openArchive();break;
      case 'openFolder':if(selectedEntries[0])enter(selectedEntries[0]);break;
      case 'preview':if(selectedEntries[0])await viewEntry(selectedEntries[0]);break;
      case 'extract':case 'delete':case 'add':await action(id);break;
      case 'extractAll':selected=[];await action('extract');break;
      case 'scanAll':await action('scan');break;
      case 'scanSelection':{
        if(!archive||busy||sample)return;
        const ids=scanEntryIds(allEntries,selectedEntries);
        if(!ids.length){notice='No files in this selection to scan.';return;}
        scanSelection=ids;selectionScanReport=null;
        scanSubject=ids.length===1?`${archive.name} › ${allEntries.find(e=>e.id===ids[0])?.name}`:`${ids.length} selected files · ${archive.name}`;
        await show('scanner');void scanArchive();break;
      }
      case 'selectAll':selected=rows.map(e=>e.id);break;
      case 'copyNames':await copyContext(selectedEntries.map(e=>e.name.replace(/\/$/,'').split('/').pop()).join('\n'));break;
      case 'copyPaths':await copyContext(selectedEntries.map(e=>e.name).join('\n'));break;
      case 'copyArchive':if(archive)await copyContext(archive.path);break;
      case 'archiveInfo':await show('info');break;
      case 'entryInfo':{
        infoEntries=[...selectedEntries];const ids=new Set(selectedIds());infoMembers=allEntries.filter(e=>ids.has(e.id));await show('entryInfo');break;
      }
    }
  }

  async function cancelScan(){if(!scanningJob)return;await command('cancel_job',{jobId:scanningJob}).catch(e=>error=String(e));}
  async function loadResearchProfile(info=archive){if(!info||sample||!isDesktop())return;if(!info.entries.some(e=>/\.tex$/i.test(e.name))&&!(['gzip','bzip2','xz','zstd'].includes(info.format.toLowerCase())&&info.entries.filter(e=>!e.is_dir).length===1)){profileGeneration++;paperLoading=false;return;}const generation=++profileGeneration;const currentPassword=password;paperLoading=true;profileError='';try{const result=await command<ResearchProfile>('research_profile',{path:info.path,password:currentPassword||null});if(generation===profileGeneration&&archive?.identity===result.archive_identity&&archive?.path===info.path&&password===currentPassword)researchProfile=result;}catch{if(generation===profileGeneration)profileError='Research metadata could not be read. Source files remain available for bounded preview.';}finally{if(generation===profileGeneration)paperLoading=false;}}
  async function copyCli(){if(!archive)return;const text=`maxrar-cli list ${shellQuote(archive.path)} --json`;try{await navigator.clipboard.writeText(text);notice='CLI command copied.';}catch{previewTitle='CLI command';preview=text;await show('preview');}}
  function toggleFavorite(){if(!archive)return;favorites=favorites.includes(archive.path)?favorites.filter(p=>p!==archive!.path):[...favorites,archive.path];persist();menu='';}
  function loadSample(){archive={path:'Sample / website-project.zip',name:'website-project.zip',identity:'sample-inventory-not-a-real-archive',format:'ZIP',total_size:44145049,compressed_size:14380928,comment:'',capabilities:{},findings:[],entries:[['src/',0,true],['assets/',0,true],['docs/',0,true],['README.md',4200,false],['package.json',1100,false],['setup.sh',2300,false],['private-backup.zip',13421772,false],['src/main.ts',1864,false],['docs/getting-started.md',8120,false]].map(([name,size,is_dir],i)=>({id:String(i),name:String(name),size:Number(size),compressed_size:Number(size)*0.6,is_dir:Boolean(is_dir),is_encrypted:false,modified:'2026-10-14T14:32:00'}))};sample=true;notice='Sample workspace · illustration only. No archive has been opened or scanned.';}
  async function projectLink(destination:ProjectLink){menu='';try{await command('open_project_link',{destination});}catch(e){notice='Could not open project link: '+String(e);}}
  async function about(check=false){await show('about');if(check)void checkUpdates();}
  function automaticUpdates(value:boolean){autoUpdates=value;localStorage.setItem('maxrar.updates.automatic',String(value));}
  async function initializeReleases(){
    autoUpdates=localStorage.getItem('maxrar.updates.automatic')!=='false';lastUpdateAttempt=Number(localStorage.getItem('maxrar.updates.lastAttempt')||0);
    if(!isDesktop()){updateView={stage:'unavailable'};return;}
    try{releaseInfo=await command<ReleaseInfo>('release_info');if(!releaseInfo.configured)updateView={stage:'unavailable'};else if(shouldAutoCheck(autoUpdates,true,lastUpdateAttempt))void checkUpdates(false);}
    catch(e){updateView={stage:'error',error:String(e),retry:'check'};}
  }
  async function checkUpdates(manual=true){
    if(updateBusy(updateView.stage))return;
    if(!manual&&!shouldAutoCheck(autoUpdates,releaseInfo.configured,lastUpdateAttempt))return;
    updateView={stage:'checking'};
    try{
      if(!isDesktop())throw new Error('Update checks require the desktop app.');
      releaseInfo=await command<ReleaseInfo>('release_info');
      if(!releaseInfo.configured){updateView={stage:'unavailable'};return;}
      lastUpdateAttempt=Date.now();localStorage.setItem('maxrar.updates.lastAttempt',String(lastUpdateAttempt));
      const result=await command<{version:string;notes:string}|null>('check_update');
      updateView=result?{stage:'available',version:result.version,notes:result.notes}:{stage:'current'};
    }catch(e){updateView={stage:'error',error:String(e),retry:'check'};}
  }
  async function downloadUpdate(){
    if(updateBusy(updateView.stage))return;
    const info={version:updateView.version,notes:updateView.notes};updateView={...info,stage:'downloading',downloaded:0};
    try{await command('download_update');updateView={...info,stage:'ready'};}
    catch(e){updateView={...info,stage:'error',error:String(e),retry:'download'};}
  }
  async function installUpdate(){
    if(activeJobs.length||busy||updateBusy(updateView.stage))return;
    const previous=updateView;updateView={...previous,stage:'installing'};busy=true;
    try{await command('install_update');}catch(e){updateView={...previous,stage:'error',error:String(e),retry:'install'};}finally{busy=false;}
  }
  function menuItems(name:string):{label:string;shortcut?:string;action:()=>void;disabled?:boolean}[]{switch(name){case 'File':return [{label:'Open archive…',shortcut:'Ctrl/⌘ O',action:openArchive},{label:'Create archive…',shortcut:'Ctrl/⌘ N',action:()=>action('add'),disabled:!isDesktop()},{label:'Close archive',action:()=>{archive=null;selected=[];menu='';},disabled:!archive}];case 'Commands':return actions.filter(a=>['extract','test','view','delete','find','comment'].includes(a.id)).map(a=>({label:a.label,action:()=>action(a.id),disabled:disabled(a.id)}));case 'Tools':return [{label:'Archive wizard…',action:()=>action('wizard')},{label:'Scan archive',action:()=>action('scan')},{label:'Copy CLI command',action:copyCli,disabled:!archive}];case 'Favorites':return [{label:favorites.includes(archive?.path||'')?'Remove current archive':'Add current archive',action:toggleFavorite,disabled:!archive},...favorites.map(path=>({label:path.split(/[\\/]/).pop()||path,action:()=>{menu='';void openPath(path);}}))];case 'Options':return [{label:'Workspace preferences…',action:()=>show('preferences')},{label:inspector?'Hide inspector':'Show inspector',action:()=>{inspector=!inspector;persist();menu='';}},{label:compact?'Comfortable rows':'Compact rows',action:()=>{compact=!compact;persist();menu='';}}];default:return [{label:'User manual',action:()=>projectLink('manual')},{label:'Keyboard shortcuts',action:()=>show('shortcuts')},{label:'Check for updates…',action:()=>about(true)},{label:'Report an issue',action:()=>projectLink('issues')},{label:'Desktop integration…',action:()=>show('shell')},{label:'About MaxRAR',action:()=>about()}];}}
  onMount(()=>{void initializeReleases();const updateTimer=setInterval(()=>{if(['idle','current','error','unavailable'].includes(updateView.stage))void checkUpdates(false);},60*60*1000);let releaseUnlisten:(()=>void)|undefined;if(isDesktop())listen<{stage:'downloading'|'verifying';downloaded:number;total:number|null}>('maxrar://update-progress',e=>{if(['downloading','verifying'].includes(updateView.stage))updateView={...updateView,...e.payload};}).then(fn=>releaseUnlisten=fn);try{const p=JSON.parse(localStorage.getItem('maxrar.workspace')||localStorage.getItem('wonrar.workspace')||'{}');theme=p.theme||'system';compact=p.compact||false;sortKey=p.sortKey||'name';ascending=p.ascending??true;dateFormat=p.dateFormat||'locale';model=p.model||'';favorites=p.favorites||[];inspector=p.inspector??true;persist();}catch{}if(import.meta.env.DEV&&!isDesktop()&&new URLSearchParams(location.search).get('demo')==='1')loadSample();let unlisten:(()=>void)|undefined;if(isDesktop())listen<Job>('wonrar://job',(e)=>{const i=jobs.findIndex(j=>j.id===e.payload.id);jobs=i<0?[...jobs,e.payload]:jobs.map((j,k)=>k===i?e.payload:j);}).then(fn=>unlisten=fn);if(isDesktop()){void initializeShellRequest();listen<string>('wonrar://open-archive',e=>openPath(e.payload));import('@tauri-apps/api/webviewWindow').then(({getCurrentWebviewWindow})=>getCurrentWebviewWindow().onDragDropEvent(e=>{if(e.payload.type==='drop'&&e.payload.paths[0])openPath(e.payload.paths[0]);}));}return()=>{unlisten?.();releaseUnlisten?.();clearInterval(updateTimer);};});
  async function rowKey(e:KeyboardEvent,entry:Entry){if(e.key==='ContextMenu'||(e.shiftKey&&e.key==='F10')){keyboardContext(e,entry);return;}if(e.key==='Enter'){e.preventDefault();enter(entry);return;}const index=rows.findIndex(row=>row.id===entry.id);const next=e.key==='ArrowDown'?Math.min(rows.length-1,index+1):e.key==='ArrowUp'?Math.max(0,index-1):e.key==='Home'?0:e.key==='End'?rows.length-1:-1;if(next<0)return;e.preventDefault();selected=e.shiftKey?[...new Set([...selected,rows[next].id])]:[rows[next].id];if(fileScroller){const top=next*rowHeight;if(top<fileScroller.scrollTop)fileScroller.scrollTop=top;if(top+rowHeight>fileScroller.scrollTop+viewportHeight)fileScroller.scrollTop=top+rowHeight-viewportHeight;}await tick();(document.querySelector('[data-entry-id="'+CSS.escape(rows[next].id)+'"]') as HTMLElement)?.focus();}
  async function menuKey(e:KeyboardEvent,name:string){if(e.key==='ArrowDown'){e.preventDefault();menu=name;await tick();(document.querySelector('.menu-popup button:not(:disabled)') as HTMLElement)?.focus();}if(e.key==='ArrowLeft'||e.key==='ArrowRight'){e.preventDefault();const index=(menuNames.indexOf(name)+(e.key==='ArrowRight'?1:menuNames.length-1))%menuNames.length;menu=menuNames[index];(document.querySelectorAll('.menu-root>button')[index] as HTMLElement)?.focus();}}
  function popupKey(e:KeyboardEvent){if(!['ArrowDown','ArrowUp','Home','End'].includes(e.key))return;e.preventDefault();const buttons=Array.from((e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('button:not(:disabled)'));const index=buttons.indexOf(document.activeElement as HTMLButtonElement);buttons[e.key==='Home'?0:e.key==='End'?buttons.length-1:(index+(e.key==='ArrowDown'?1:buttons.length-1))%buttons.length]?.focus();}
  function keyboard(e:KeyboardEvent){if(contextMenu)return;if(e.key==='Escape'){menu='';if(dialog)closeDialog();}if(dialog)return;const id=shortcutAction(e);if(!id)return;if(id==='select_all'&&(e.target as HTMLElement).closest('input,textarea,[contenteditable=true]'))return;e.preventDefault();if(id==='open')void openArchive();else if(id==='create')void action('add');else if(id==='find')searchInput?.focus();else selected=rows.map(entry=>entry.id);}
</script>

<svelte:window oncontextmenu={event=>{if(!(event.target as HTMLElement).closest('input,textarea,pre,[contenteditable=true]'))event.preventDefault();}} onkeydown={keyboard} onclick={(e)=>{if(!(e.target as HTMLElement).closest('.menu-root'))menu='';}} />
<div class="application" data-theme={theme}>
  <header class="titlebar"><div class="app-identity"><img src={appIcon} alt=""/><strong>MaxRAR</strong><span class="title-divider"></span><span class="window-title">{archive?.name||'Archive workspace'}</span></div>{#if sample}<span class="platform-label">SAMPLE WORKSPACE</span>{/if}</header>
  <nav class="menubar" aria-label="Application menus">{#each menuNames as name}<div class="menu-root"><button class:active={menu===name} aria-haspopup="menu" aria-expanded={menu===name} onkeydown={e=>menuKey(e,name)} onclick={()=>menu=menu===name?'':name}>{name}</button>{#if menu===name}<div class="menu-popup" role="menu" tabindex="-1" onkeydown={popupKey}>{#each menuItems(name) as item}<button role="menuitem" disabled={item.disabled} onclick={item.action}>{item.label}<span>{item.shortcut||''}</span></button>{/each}</div>{/if}</div>{/each}<span class="menu-end">Classic power. Clearer control.</span></nav>
  <div class="toolbar" role="toolbar" aria-label="Archive commands">{#each actions as item}<button class="tool" disabled={disabled(item.id)} title={disabled(item.id)?(!archive?'Open an archive to use this command':item.id==='delete'?'Archive deletion is unavailable for this format or selection':item.hint):item.hint} onclick={()=>action(item.id)}><img src={'/icons/'+item.id+'.svg'} alt=""/><span>{item.label}</span>{#if item.id==='scan'}<small>Built-in · offline</small>{/if}</button>{/each}</div>
  <div class="navigation"><div class="nav-buttons"><button title="Archive root" aria-label="Archive root" disabled={!folder} onclick={()=>{folder='';selected=[];}}>←</button><button title="Up one folder" aria-label="Up one folder" disabled={!folder} onclick={()=>{folder=folder.split('/').slice(0,-1).join('/');selected=[];}}>↑</button></div><div class="breadcrumb"><img src="/icons/archive.svg" alt=""/><button disabled={!archive} onclick={()=>folder=''}>{archive?.name||'No archive open'}</button>{#each folder.split('/').filter(Boolean) as part}<span>›</span><span>{part}</span>{/each}</div><label class="search"><img src="/icons/find.svg" alt=""/><input bind:this={searchInput} bind:value={query} disabled={!archive} placeholder="Find in archive…" aria-label="Find files in archive" oninput={()=>{rowScroll=0;}}/>{#if query}<button onclick={()=>query=''} aria-label="Clear search">×</button>{:else}<kbd>⌘ F</kbd>{/if}</label><button class:pressed={inspector} class="inspector-toggle" title="Toggle archive inspector" aria-label="Toggle archive inspector" onclick={()=>{inspector=!inspector;persist();}}>☷</button></div>
  {#if error}<div class="banner error" role="alert"><strong>Operation could not finish</strong><span>{error}</span><button onclick={()=>error=''} aria-label="Dismiss error">×</button></div>{/if}
  <main class:without-inspector={!inspector}>
    <aside class="tree" aria-label="Archive folders"><div class="pane-caption">EXPLORER</div>{#if archive}<button oncontextmenu={event=>openContext(event)} onkeydown={event=>keyboardContext(event)} class:chosen={!folder} class="tree-root" onclick={()=>{folder='';query='';}}><span class="chevron">⌄</span><img src="/icons/archive.svg" alt=""/><span>{archive.name}</span></button>{#each folders as path}<button class="tree-folder" class:chosen={folder===path} style:padding-left={(path.split('/').length*14+16)+'px'} oncontextmenu={event=>{const entry=browsingEntries.find(e=>e.name.replace(/\/$/,'')===path);if(entry)openContext(event,entry);}} onkeydown={event=>{const entry=browsingEntries.find(e=>e.name.replace(/\/$/,'')===path);if(entry)keyboardContext(event,entry);}} onclick={()=>{folder=path;query='';selected=[];rowScroll=0;}}><span class="chevron">›</span><img src="/icons/folder.svg" alt=""/><span>{path.split('/').pop()}</span></button>{/each}{:else}<div class="tree-empty"><img src="/icons/archive.svg" alt=""/><span>No archive open</span></div>{/if}<div class="tree-bottom"><span class="status-dot"></span> Local by default</div></aside>
    <section class="central"><div class="file-pane" role="region" aria-label="Archive file browser" oncontextmenu={event=>openContext(event)}><div class="table-head" role="row">{#each [{id:'name',name:'Name'},{id:'size',name:'Size'},{id:'type',name:'Type'},{id:'modified',name:'Modified'}] as col}<button class={'col-'+col.id} onclick={()=>sort(col.id)} aria-label={'Sort by '+col.name+(sortKey===col.id?(ascending?', ascending':', descending'):'')}>{col.name}{#if sortKey===col.id}<span>{ascending?'↑':'↓'}</span>{/if}</button>{/each}</div>
      {#if !archive}<div class="welcome"><div class="welcome-art"><img src={appIcon} alt=""/></div><span class="eyebrow">ARCHIVE WORKSPACE</span><h1>Open an archive</h1><p>Browse, inspect, and extract your files.</p><button class="primary open-button" onclick={openArchive}><img src="/icons/folder.svg" alt=""/>Open an archive<span>⌘ O</span></button><button class="text-button" disabled={!isDesktop()} onclick={()=>action('add')}>or create a new archive →</button><div class="welcome-formats"><span>ZIP</span><span>RAR</span><span>7z</span><span>TAR</span><span>GZ / BZ2 / XZ / ZST</span></div>{#if !isDesktop()}<div class="preview-note">Open the MaxRAR app to work with archives.</div>{/if}</div>
      {:else}<div class="file-scroll" bind:this={fileScroller} onscroll={e=>{rowScroll=e.currentTarget.scrollTop;contextMenu=null;}} bind:clientHeight={viewportHeight}><div role="grid" tabindex="0" onkeydown={event=>{if(event.target===event.currentTarget)keyboardContext(event);}} aria-label="Archive contents" aria-rowcount={rows.length} aria-multiselectable="true" class="file-table"><div style:height={start*rowHeight+'px'}></div>{#each visibleRows as entry(entry.id)}<button role="row" data-entry-id={entry.id} aria-selected={selected.includes(entry.id)} class="file-row" class:selected={selected.includes(entry.id)} class:compact style:height={rowHeight+'px'} onclick={e=>select(entry,e)} oncontextmenu={e=>openContext(e,entry)} ondblclick={()=>enter(entry)} onkeydown={e=>rowKey(e,entry)}><span class="filename"><img src={'/icons/'+iconFor(entry)+'.svg'} alt=""/><span>{query?entry.name:entry.name.replace(/\/$/,'').split('/').pop()}</span>{#if entry.is_encrypted}<EncryptedBadge/>{/if}</span><span class="filesize">{entry.is_dir?'—':bytes(entry.size)}</span><span class="filetype">{kind(entry)}</span><span class="filedate">{formatModified(entry.modified,dateFormat)}</span></button>{/each}<div style:height={(rows.length-end)*rowHeight+'px'}></div></div>{#if rows.length===0}<div class="no-results"><img src="/icons/find.svg" alt=""/><strong>{query?'No matching files':'This folder is empty'}</strong><p>{query?'Try a different filename or path.':'Choose another folder in the explorer.'}</p></div>{/if}</div>{/if}</div>
    <section class="tasks" aria-label="Task queue"><div class="task-heading"><strong>Tasks <span>{jobs.length?`(${jobs.length})`:''}</span></strong><span>{activeJobs.length?`${activeJobs.length} in progress`:'Recent operations'}</span>{#if jobs.length&& !activeJobs.length}<button class="text-button" onclick={()=>jobs=[]}>Clear completed</button>{/if}</div>{#if !jobs.length}<div class="tasks-empty"><span class="queue-mark">✓</span><span>No active tasks</span></div>{:else}<div class="task-list">{#each jobs as job}<div class="task-row"><img src="/icons/archive.svg" alt=""/><div class="job-description"><strong>{job.kind}</strong><small>{job.message||job.status}</small></div><span class="job-state">{job.status==='running'&&job.progress!==null?Math.round(job.progress*100)+'%':job.status}</span>{#if job.status==='running'}<progress value={job.progress??undefined} max="1" aria-label={job.kind+' progress'}></progress><button onclick={()=>command('cancel_job',{jobId:job.id}).catch(e=>error=String(e))}>Cancel</button>{:else}<span class="job-result">{job.status==='completed'?'✓':'—'}</span>{/if}{#if job.receipt}<button class="text-button" onclick={()=>{previewTitle='Task receipt';preview=JSON.stringify(job.receipt,null,2);show('preview');}}>Receipt</button>{/if}</div>{/each}</div>{/if}</section></section>
    {#if inspector}<aside class="inspector" aria-label="Archive inspector"><div class="inspector-heading"><h2>What's inside?</h2><button onclick={()=>{inspector=false;persist();}} aria-label="Collapse inspector">⌄</button></div><div class="inspector-content">
      {#if archive}{#key archive.identity}<ArchiveOverview {archive} sourceHintIds={researchProfile?.main_candidates.map(c=>c.entry_id)||[]} scan={scannerReport} scanAvailable={isDesktop()&&!sample} {busy} onpreview={viewEntry} onlocate={entry=>{folder=entry.name.includes('/')?entry.name.slice(0,entry.name.lastIndexOf('/')):'';query='';selected=[entry.id];rowScroll=0;void tick().then(()=>{const index=rows.findIndex(e=>e.id===entry.id);if(fileScroller&&index>=0)fileScroller.scrollTop=index*rowHeight;});}} onfolder={path=>{folder=path;query='';selected=[];}} onscan={()=>{if(scannerReport){scanSelection=null;scanSubject=archive?.name||'';void show('scanner');}else void action('scan');}}/>{/key}
      {:else}<div class="inspector-empty"><img src="/icons/archive.svg" alt="" width="32" height="32"/><h3>What's inside?</h3><p>Open an archive for a quick overview.</p></div>{/if}
    </div><PromotionSlot/></aside>{/if}
  </main>
  <footer class="statusbar"><button disabled={!archive} onclick={copyCli}><span class="copy-glyph">▢</span>Copy CLI command</button><span class="status-separator"></span><span>{selected.length?`${selected.length} item${selected.length===1?'':'s'} selected`:archive?'Ready':'Ready to explore'}</span><span class="status-message" aria-live="polite">{notice}</span><span class="status-count">{archive?`${allEntries.length} items`:'Windows · macOS · Linux'}</span>{#if archive}<span class="status-separator"></span><span>{bytes(archive.total_size)} (in archive)</span>{/if}</footer>
  {#if contextMenu}<ContextMenu x={contextMenu.x} y={contextMenu.y} items={contextItems} onchoose={contextAction} ondismiss={dismissContext}/>{/if}
  {#if dialog}<dialog bind:this={dialogEl} oncancel={e=>{e.preventDefault();closeDialog();}} class:wide-dialog={dialog==='preview'} class:extract-dialog={dialog==='extract'} class:test-dialog={dialog==='test'} class:about-dialog={dialog==='about'} class:shortcuts-dialog={dialog==='shortcuts'} class:scan-dialog={dialog==='scanner'} class:create-dialog={dialog==='create'}><div class="dialog-heading"><h2>{({extract:'Extract files',test:'Test archive',create:'Create an archive',delete:'Delete selected entries?',preview:previewTitle,comment:'Archive comment',scanner:scanSelection===null?'Archive scan':'Selection scan',wizard:'Archive wizard',info:'Archive information',entryInfo:infoEntries.length===1?(infoEntries[0].is_dir?'Folder information':'File information'):'Selection information',preferences:'Workspace preferences',shell:'Desktop integration',shortcuts:'Keyboard shortcuts',about:'About MaxRAR',password:'Archive password'} as Record<string,string>)[dialog]}</h2><button onclick={closeDialog} aria-label="Close dialog">×</button></div><div class="dialog-content">
    {#if dialog==='extract'}<ExtractPanel name={archive?.name||'Archive'} sourceFiles={extractSource.files} sourceBytes={extractSource.bytes} bind:destination bind:policy bind:password bind:openAfter encrypted={allEntries.some(e=>e.is_encrypted)} plan={extractionPlan} {planning} {busy} {sample} failure={planError} review={extractionReview} onchange={chooseFolder} onextract={extract} oncancel={closeDialog}/>
    {:else if dialog==='test'}<TestPanel name={testName} files={testFiles} format={testFormat} encrypted={testEncrypted} running={!!testJob} job={jobs.find(j=>j.id===testJob)} receipt={testReceipt} failure={testError} passwordRequired={testNeedsPassword} onclose={closeDialog} oncancel={cancelTest} onretry={()=>performTest()} onpassword={testPassword}/>
    {:else if dialog==='shell'}<ShellIntegration/>{:else if dialog==='create'}<CreatePanel initialSources={createSources} available={isDesktop()} job={createJob} oncancel={()=>{if(createJob)void command('cancel_job',{jobId:createJob.id}).catch(e=>error=String(e));}} onclose={closeDialog} oncreate={createArchive}/>
    {:else if dialog==='delete'}<p>Delete {selectedIds().length} selected archive entries from <strong>{archive?.name}</strong>? This changes the archive.</p><div class="dialog-actions"><button onclick={closeDialog}>Cancel</button><button class="danger" onclick={deleteSelected}>Delete entry</button></div>
    {:else if dialog==='preview'}<div class="preview-meta">Bounded text preview · rendered as plain text · no execution</div><pre class="file-preview">{preview}</pre>
    {:else if dialog==='comment'}<label class="field-label" for="comment">Archive comment</label><textarea id="comment" bind:value={comment} readonly={!capability('comment')} rows="8" placeholder="This archive has no comment."></textarea>{#if !capability('comment')}<p class="muted">Comment editing is unavailable for this archive/backend.</p>{/if}<div class="dialog-actions"><button onclick={closeDialog}>Close</button><button class="primary" disabled={busy||!capability('comment')} onclick={saveComment}>Save comment</button></div>
    {:else if dialog==='scanner'}<ScanPanel report={activeScanReport} selectionCount={scanSelection?.length||0} running={!!scanningJob} job={jobs.find(j=>j.id===scanningJob)} name={scanSubject||archive?.name||''} failure={error} available={!!archive&&isDesktop()&&!sample} disabled={busy} oncancel={cancelScan} onclose={closeDialog} onscan={scanArchive}/>
    {:else if dialog==='wizard'}<p class="body-copy">Choose your task. You’ll review every operation before it runs.</p><button class="wizard-option" disabled={!isDesktop()} onclick={()=>{closeDialog();action('add');}}><img src="/icons/add.svg" alt=""/><span><strong>Create a new archive</strong><small>Package selected files for sharing</small></span><span>→</span></button><button class="wizard-option" onclick={()=>{closeDialog();openArchive();}}><img src="/icons/archive.svg" alt=""/><span><strong>Open and inspect an archive</strong><small>Browse files, findings, and evidence</small></span><span>→</span></button><button class="wizard-option" disabled={!archive||sample} onclick={()=>{closeDialog();action('extract');}}><img src="/icons/extract.svg" alt=""/><span><strong>Extract the current archive</strong><small>Review destinations and conflicts</small></span><span>→</span></button>
    {:else if dialog==='entryInfo'}<dl class="info-list">{#if infoEntries.length===1}<div><dt>Name</dt><dd>{infoEntries[0].name.replace(/\/$/,'').split('/').pop()}</dd></div><div><dt>Path in archive</dt><dd>{infoEntries[0].name}</dd></div><div><dt>Type</dt><dd>{kind(infoEntries[0])}</dd></div>{#if !infoEntries[0].is_dir}<div><dt>Original size</dt><dd>{bytes(infoEntries[0].size)}</dd></div><div><dt>Compressed size</dt><dd>{archive?.format.toLowerCase().startsWith('tar')?'Not recorded':bytes(infoEntries[0].compressed_size)}</dd></div><div><dt>Modified</dt><dd>{formatModified(infoEntries[0].modified,dateFormat)}</dd></div><div><dt>Password protected</dt><dd>{infoEntries[0].is_encrypted?'Yes':'No'}</dd></div>{/if}{:else}<div><dt>Selected items</dt><dd>{infoEntries.length}</dd></div>{/if}{#if infoEntries.length!==1||infoEntries[0].is_dir}<div><dt>Contained files</dt><dd>{infoMembers.filter(e=>!e.is_dir).length}</dd></div><div><dt>Original size</dt><dd>{bytes(infoMembers.reduce((sum,e)=>sum+e.size,0))}</dd></div>{/if}<div><dt>Archive</dt><dd>{archive?.name}</dd></div></dl><div class="dialog-actions"><button class="primary" onclick={closeDialog}>Close</button></div>
    {:else if dialog==='info'}<dl class="info-list"><div><dt>Archive</dt><dd>{archive?.name}</dd></div><div><dt>Path</dt><dd>{archive?.path}</dd></div><div><dt>Format</dt><dd>{archive?.format.toLowerCase()==='mrar'?'MRAR · Experimental':archive?.format}</dd></div><div><dt>Original size</dt><dd>{bytes(archive?.total_size||0)}</dd></div><div><dt>Compressed size</dt><dd>{bytes(archive?.compressed_size||0)}</dd></div><div><dt>Entries</dt><dd>{allEntries.length}</dd></div><div><dt>Encrypted entries</dt><dd>{allEntries.filter(e=>e.is_encrypted).length}</dd></div></dl><h3>Backend capabilities</h3><div class="capabilities">{#each Object.entries(archive?.capabilities||{}) as [name,enabled]}<span class:available={enabled}>{enabled?'✓':'—'} {name.replaceAll('_',' ')}</span>{/each}</div><p class="muted">Capabilities describe this format and installed backend. Integrity checks are separate from malware scanning.</p>
    {:else if dialog==='preferences'}<label class="field-label" for="theme">Appearance</label><select id="theme" bind:value={theme} onchange={persist}><option value="system">Use system appearance</option><option value="light">Light</option><option value="dark">Dark</option></select><label class="check-option"><input type="checkbox" bind:checked={compact} onchange={persist}/>Compact file rows</label><label class="field-label" for="dates">Date display</label><select id="dates" bind:value={dateFormat} onchange={persist}><option value="locale">Local date format</option><option value="iso">ISO date and time</option></select><div class="dialog-actions"><button class="primary" onclick={()=>{persist();closeDialog();}}>Done</button></div>
    {:else if dialog==='shortcuts'}<ShortcutsPanel onclose={closeDialog} apple={/Mac|iPhone|iPad/.test(navigator.platform)}/>
    {:else if dialog==='password'}<PasswordPanel oncancel={closeDialog} oncontinue={continuePassword}/>{:else if dialog==='about'}<AboutPanel info={releaseInfo} update={updateView} automatic={autoUpdates} jobsActive={!!activeJobs.length||busy} oncheck={()=>checkUpdates()} ondownload={downloadUpdate} oninstall={installUpdate} onautomatic={automaticUpdates} onlink={projectLink}/>{/if}
  </div>{#if error&&dialog!=='create'&&dialog!=='test'}<div class="dialog-error" role="alert">{error}</div>{/if}</dialog>{/if}
</div>
