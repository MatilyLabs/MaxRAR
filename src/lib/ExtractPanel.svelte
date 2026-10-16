<script lang="ts">
  import { bytes } from './bridge';
  import { destinationParts, extractionSummary } from './extraction';
  import type { ExtractionView } from './extraction';
  let { name, sourceFiles = 0, sourceBytes = 0, destination = $bindable(''), policy = $bindable('skip'),
    password = $bindable(''), openAfter = $bindable(true), encrypted = false, plan = null,
    planning = false, busy = false, sample = false, failure = '', review = '', onchange, onextract, oncancel
  }: { name: string; sourceFiles?: number; sourceBytes?: number; destination?: string; policy?: string;
    password?: string; openAfter?: boolean; encrypted?: boolean; plan?: ExtractionView | null;
    planning?: boolean; busy?: boolean; sample?: boolean; failure?: string; review?: string;
    onchange: () => void; onextract: () => void; oncancel: () => void } = $props();
  const target = $derived(destinationParts(destination));
  const summary = $derived(extractionSummary(plan));
  const checked = $derived(!!plan && !planning && !failure && (plan.requested_destination ?? plan.destination) === destination);
  const blocked = $derived(summary.blocked.length > 0);
</script>

<section class="extract-panel" aria-label="Extraction options">
  <div class="transfer" aria-label="Archive to destination folder">
    <div class="endpoint"><img src="/icons/archive.svg" alt=""/><div class="endpoint-copy"><strong title={name}>{name}</strong><span>{sourceFiles} {sourceFiles === 1 ? 'file' : 'files'} · {bytes(sourceBytes)}</span></div></div>
    <span class="arrow" aria-hidden="true">→</span>
    <div class="endpoint"><img src="/icons/folder.svg" alt=""/><div class="endpoint-copy"><strong title={destination}>{target.name}</strong><span>{checked && plan?.destination_exists !== undefined ? plan.destination_exists ? 'Existing folder' : 'New folder' : 'Destination folder'}</span></div></div>
  </div>
  <div class="destination-row"><span class="destination-caption">Destination</span><div class="destination-control"><div class="destination-crumb" title={destination}><img src="/icons/folder.svg" alt=""/>{#if target.parent}<span class="parent-folder">{target.parent}</span><span class="crumb-chevron" aria-hidden="true">›</span>{/if}<strong>{target.name}</strong></div><button class="secondary" onclick={onchange} disabled={busy || sample}>Change…</button></div></div>
  {#if encrypted}<label class="password-label" for="extract-password">Archive password</label><input id="extract-password" type="password" bind:value={password} disabled={busy} autocomplete="off"/>{/if}
  <div class="result-group" class:has-conflicts={checked && summary.conflicts.length > 0 && !blocked}>
  <div class="status" class:warning={checked && summary.conflicts.length > 0} class:danger={blocked || !!failure} role="status">
    {#if planning}<span class="spinner" aria-hidden="true"></span><strong>Checking destination…</strong>
    {:else if failure}<span aria-hidden="true">!</span><strong>Unable to check destination</strong>
    {:else if blocked}<span aria-hidden="true">!</span><strong>{summary.blocked.length} {summary.blocked.length === 1 ? 'item blocked' : 'items blocked'}</strong>
    {:else if checked && summary.conflicts.length}<span aria-hidden="true">!</span><strong>{summary.conflicts.length} {summary.conflicts.every(e=>!e.is_dir) ? 'file' : 'item'}{summary.conflicts.length === 1 ? '' : 's'} already exist{summary.conflicts.length === 1 ? 's' : ''}</strong>
    {:else if checked}<span class="check" aria-hidden="true">✓</span><strong>{summary.writing ? 'Ready to extract' : 'Nothing to extract'}</strong>
    {:else}<span aria-hidden="true">ⓘ</span><strong>{sample ? 'Preview only' : 'Choose a destination'}</strong>{/if}
  </div>
  {#if failure || review}<p class="error" role="alert">{failure || review}</p>{/if}
  {#if summary.conflicts.length && (plan?.requested_destination ?? plan?.destination) === destination}
    <div class="conflicts"><div class="conflict-names">{#each summary.conflicts.slice(0,2) as entry}<span title={entry.output_name}><img src={'/icons/'+(entry.is_dir?'folder':'document')+'.svg'} alt=""/>{entry.source_name}</span>{/each}{#if summary.conflicts.length>2}<small>+{summary.conflicts.length-2} more</small>{/if}</div>
    </div>
  {/if}
  </div>
  {#if summary.conflicts.length && (plan?.requested_destination ?? plan?.destination) === destination}
    <div class="conflict-options">    <fieldset disabled={busy}><legend>For existing files</legend>{#each [{id:'skip',label:'Skip existing'},{id:'keep_both',label:'Keep both'},{id:'replace',label:'Replace'}] as option}<label><input type="radio" name="extract-policy" value={option.id} bind:group={policy}/>{option.label}</label>{/each}</fieldset>
    {#if checked}<p class="outcome">{summary.outcome}</p>{#if policy==='replace'}<p class="replace-note">Existing files will be overwritten.</p>{/if}{/if}</div>
  {/if}
  {#if blocked}<ul class="blocked-list">{#each summary.blocked.slice(0,3) as entry}<li><strong>{entry.source_name}</strong><span>{entry.reason || 'Cannot extract this entry.'}</span></li>{/each}{#if summary.blocked.length>3}<li>+{summary.blocked.length-3} more in Details</li>{/if}</ul>{/if}
  <details class="extract-details"><summary>Details</summary><label for="extract-destination">Full destination path</label><input id="extract-destination" bind:value={destination} disabled={busy} placeholder="Destination folder…" spellcheck="false"/>
    {#if checked}<p>{summary.outcome} · {bytes(plan?.total_bytes || 0)}</p>{/if}
    {#if plan}<div class="plan-list">{#each plan.entries.slice(0,200) as entry}<div><span title={entry.source_name}>{entry.source_name}<small>→ {entry.output_name}</small>{#if entry.reason}<small>{entry.reason}</small>{/if}</span><strong class:danger={entry.action==='blocked'}>{entry.action==='skip'?'Skip':entry.action==='blocked'?'Blocked':entry.conflict?policy==='replace'?'Replace':'Keep both':'Extract'}</strong></div>{/each}</div>{#if plan.entries.length>200}<p>Showing 200 items. All items are checked before extraction.</p>{/if}{/if}
    <p class="muted">The archive and destination are checked again before writing files.</p>
  </details>
  <label class="open-folder"><input type="checkbox" bind:checked={openAfter} disabled={busy}/>Open folder when finished</label>
  <footer><button class="secondary" onclick={oncancel} disabled={busy}>Cancel</button><button class="primary" onclick={onextract} disabled={busy || sample || !checked || blocked || !summary.writing}>{busy ? 'Extracting…' : checked && summary.conflicts.length ? summary.button : 'Extract'}</button></footer>
</section>

<style>
  .extract-panel{font-size:13px;color:var(--text);line-height:1.45}
  .transfer{display:grid;grid-template-columns:minmax(0,1fr) 24px minmax(0,1fr);align-items:center;gap:14px;padding:4px 0 22px;border-bottom:1px solid var(--line)}
  .endpoint{display:flex;align-items:center;gap:12px;min-width:0}
  .endpoint img{width:44px;height:44px;flex-shrink:0}
  .endpoint-copy{min-width:0;display:flex;flex-direction:column;gap:4px}
  .endpoint strong{font-size:13px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
  .endpoint-copy>span{font-size:12px;color:var(--muted)}
  .arrow{font-size:25px;color:var(--muted);text-align:center}
  .destination-row{padding:18px 0 0}
  .destination-caption{display:block;font-weight:550;margin-bottom:8px}
  .destination-control{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:10px;align-items:stretch}
  .destination-crumb{display:flex;align-items:center;gap:8px;min-width:0;height:38px;padding:0 10px;border:1px solid var(--line);border-radius:6px;background:var(--surface)}
  .destination-crumb img{height:20px;width:20px;flex-shrink:0}
  .parent-folder{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:40%;color:var(--muted)}
  .crumb-chevron{color:var(--muted);flex-shrink:0}
  .destination-crumb strong{font-weight:450;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .extract-panel button{font-size:13px;padding:8px 14px;min-height:36px;line-height:20px;border-radius:6px}
  .extract-panel .secondary{border:1px solid var(--line);background:var(--surface);font-weight:450}
  .extract-panel .secondary:hover:not(:disabled){background:var(--soft)}
  .destination-control button{height:38px;min-width:88px;white-space:nowrap}
  .result-group{margin-top:14px}
  .status{display:flex;align-items:center;gap:9px;padding:4px 0;min-height:30px}
  .status strong{font-weight:450}
  .status>span:not(.spinner){display:grid;place-items:center;background:var(--accent);color:white;border-radius:50%;height:22px;width:22px;font-size:14px;font-weight:650;flex-shrink:0}
  .status.warning>span{background:var(--warning);color:var(--surface)}
  .has-conflicts{padding:10px 12px;border:1px solid color-mix(in srgb,var(--warning) 32%,var(--surface));border-radius:6px;background:var(--warning-soft)}
  .has-conflicts .status{padding:0 0 8px}.has-conflicts .status strong{font-weight:550}
  .danger,.status.danger,.error{color:#ba4039}.status.danger>span{background:#ba4039}
  .error{font-size:12px;margin:6px 0 12px;overflow-wrap:anywhere}
  .conflict-names{display:flex;flex-direction:column;gap:6px;font-size:12px;padding-left:1px}
  .conflict-names>span{display:flex;align-items:center;gap:13px;overflow-wrap:anywhere}
  .conflict-names img{width:18px;height:18px;flex-shrink:0}.conflict-names small{padding-left:31px;color:var(--muted)}
  .conflict-options fieldset{display:flex;flex-direction:row;flex-wrap:wrap;justify-content:space-between;gap:12px;border:0;padding:0;margin:16px 0 10px}
  .conflict-options legend{display:block;padding:0;margin-bottom:10px;font-weight:550;font-size:12px}
  fieldset label,.open-folder{display:flex;align-items:center;gap:8px}
  input[type=radio],input[type=checkbox]{width:15px;height:15px;margin:0;accent-color:var(--accent)}
  .outcome{font-size:12px;margin:10px 0;color:var(--muted)}
  .replace-note{font-size:12px;color:var(--warning);margin:4px 0 12px}
  .extract-details{padding:8px 0;margin:18px 0 10px}
  .extract-details summary{cursor:pointer;font-weight:550;list-style-position:inside}
  .extract-details label,.password-label{display:block;font-size:12px;margin:14px 0 6px}
  .extract-panel input:not([type=radio]):not([type=checkbox]){box-sizing:border-box;width:100%;font-size:12px;padding:9px;min-width:0}
  .extract-details p{font-size:12px;color:var(--muted)}
  .plan-list{max-height:180px;overflow:auto;margin:12px 0}
  .plan-list>div{display:flex;justify-content:space-between;gap:12px;padding:9px 0;border-bottom:1px solid var(--line);font-size:12px}
  .plan-list>div>span{min-width:0;overflow-wrap:anywhere}.plan-list small{display:block;color:var(--muted);margin-top:3px}.plan-list strong{font-size:11px;flex-shrink:0}
  .open-folder{font-size:12px;margin:4px 0 20px}
  footer{display:flex;justify-content:flex-end;gap:10px;border-top:1px solid var(--line);padding-top:16px}
  footer .primary{min-width:100px}footer .secondary{min-width:88px}
  .blocked-list{list-style:none;margin:0 0 12px;padding:0;font-size:12px}.blocked-list li{padding:6px 0;overflow-wrap:anywhere}.blocked-list span{display:block;color:var(--muted);margin-top:3px}
  .spinner{width:16px;height:16px;border:2px solid var(--line);border-top-color:var(--accent);border-radius:50%;animation:spin 1s linear infinite}
  @keyframes spin{to{transform:rotate(360deg)}}
  @media(prefers-reduced-motion:reduce){.spinner{animation:none}}
  @media(max-width:450px){.transfer{gap:8px;grid-template-columns:minmax(0,1fr) 18px minmax(0,1fr)}.endpoint{gap:8px}.endpoint img{width:32px;height:32px}.endpoint strong,.endpoint-copy>span{font-size:11px}.destination-control{gap:8px}.destination-control button{min-width:76px;padding:8px 10px}.destination-crumb{gap:6px;padding:0 8px}.parent-folder{max-width:30%}}
</style>
