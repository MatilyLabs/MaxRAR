<script lang="ts">
  import type { ScanReport } from './scan';
  import type { Job } from './bridge';
  import { bytes } from './bridge';
  import { findingTitle, scanOutcome } from './scan';
  let { report=null, running=false, job, name='', failure='', available=true, oncancel, onclose, onscan, disabled=false,selectionCount=0 }:
    {report?:ScanReport|null;running?:boolean;job?:Job;name?:string;failure?:string;available?:boolean;oncancel:()=>void;onclose:()=>void;onscan:()=>void;disabled?:boolean;selectionCount?:number}=$props();
  const selection=$derived(report?.scope?.kind==='selection'||selectionCount>0);
  const scopeCount=$derived(report?.scope?.selected_files||selectionCount);
  const outcome=$derived(report?scanOutcome(report):null);
  const state=$derived(running?'running':outcome?.state||'incomplete');
  const fraction=$derived(Math.max(0,Math.min(1,job?.progress??0)));
  const determinate=$derived(!!job?.progress);
  const title=$derived(running?(job?.status==='queued'?'Waiting to scan':'Scanning…'):outcome?.title||(failure?(/cancel/i.test(failure)?'Scan cancelled':'Scan interrupted'):'Ready to scan'));
  const duration=$derived(report?report.elapsed_ms<1000?`${report.elapsed_ms} ms`:`${(report.elapsed_ms/1000).toFixed(1)} s`:'');
</script>

<section class="scan-panel" data-result={state} aria-label={selection?'Selection scan':'Archive scan'}>
  {#if selection}<div class="selection-scope">Selection scan <span>· {scopeCount} selected {scopeCount===1?'file':'files'}</span></div>{/if}
  <div class="result-hero">
    <div class="result-emblem" class:working={running} aria-hidden="true">
      <svg viewBox="0 0 48 48"><path d="M24 4 40 10v12c0 10-7 17-16 22C15 39 8 32 8 22V10Z" fill="currentColor" opacity=".12"/><path d="M24 4 40 10v12c0 10-7 17-16 22C15 39 8 32 8 22V10Z" fill="none" stroke="currentColor" stroke-width="2"/>{#if state==='clear'}<path d="m16 24 6 6 11-13" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>{:else if state==='blocked'}<path d="m18 18 12 12m0-12L18 30" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>{:else}<path d="M24 16v10m0 6v1" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>{/if}</svg>
    </div>
    <div class="result-heading"><h3 aria-live="polite">{title}</h3><p class="archive-name" title={name}>{name||'Open an archive to begin'}</p></div>
  </div>

  {#if running}
    <div class="scan-live"><div class="live-label"><span>{job?.status==='queued'?'Waiting for a worker':'Checking files'}</span><strong>{determinate?Math.round(fraction*100)+'%':'Please wait'}</strong></div><progress aria-label="Scan progress" max="1" value={determinate?fraction:undefined}></progress><p title={job?.message}>{job?.message||'Preparing archive…'}</p></div>
  {:else if report && outcome}
    <div class="status-scale" role="img" aria-label={'Scan result: '+outcome.title+'. Categorical result, not a safety score.'}>
      {#each [{id:'clear',label:'No issues'},{id:'attention',label:'Caution'},{id:'blocked',label:'Unsafe entries'}] as segment}
        <div class={'scale-segment '+segment.id} class:current={state===segment.id}><span class="scale-pointer" aria-hidden="true">{state===segment.id?'▼':''}</span><span class="scale-track"></span><span class="scale-label">{segment.label}</span></div>
      {/each}
    </div>
    <div class="result-story"><strong>{outcome.reason}</strong>{#if outcome.issues.length}<p>Review the marked files before opening or running them.</p>{/if}</div>
    <div class="coverage-line" class:partial={!outcome.complete}><span aria-hidden="true">{outcome.complete?'✓':'◷'}</span><span>{report.coverage.scanned_files} of {report.coverage.total_files} {report.coverage.total_files===1?'file':'files'} checked{!outcome.complete?' · some content unchecked':''}</span><small>{duration}</small></div>
    {#if outcome.issues.length}<div class="issue-list">{#each outcome.issues as finding}<details class="issue-row"><summary><span class="issue-symbol" aria-hidden="true">!</span><span class="issue-copy"><strong title={finding.path}>{finding.path||findingTitle(finding.code)}</strong><small>{findingTitle(finding.code)}</small></span><span class="issue-label">{['error','critical','danger'].includes(finding.severity)?'Unsafe':'Caution'}</span><span class="chevron" aria-hidden="true">⌄</span></summary><div class="issue-detail"><p>{finding.message}</p><code>{finding.code}</code></div></details>{/each}</div>{/if}
    {#if outcome.notes.length}<details class="scan-notes"><summary>{outcome.notes.filter(f=>f.severity==='test').length?outcome.notes.every(f=>f.severity==='test')?'Harmless test file detected':'Test files & other notes':'Other file notes'} <span>{outcome.notes.length}</span></summary>{#each outcome.notes as note}<div><strong>{findingTitle(note.code)}</strong><small>{note.path}</small><p>{note.message}</p></div>{/each}</details>{/if}
    <details class="scan-technical"><summary>Scan details <span>Coverage &amp; scan log</span></summary><dl><div><dt>Content inspected</dt><dd>{bytes(report.coverage.decoded_bytes)}</dd></div><div><dt>Coverage</dt><dd>{report.coverage.skipped_files} skipped · {report.coverage.truncated_files} partial · {report.coverage.opaque_containers} opaque containers · {report.coverage.nested_archives} nested archives</dd></div><div><dt>Limits</dt><dd>{report.limits.max_entries} entries · {bytes(report.limits.max_entry_bytes)} per file · {bytes(report.limits.max_total_bytes)} decoded · {report.limits.max_duration_secs}s · depth {report.limits.max_depth}</dd></div><div><dt>Archive SHA-256</dt><dd><code>{report.archive_identity}</code></dd></div></dl><p>{report.message}</p>{#each report.skipped as item}<p><strong>{item.path}</strong><br/>{item.reason}</p>{/each}{#each report.warnings as warning}<p>{warning}</p>{/each}</details>
  {:else}<div class="scan-empty">{failure||(!available?'Open an archive in the desktop app to scan.':'Built-in checks run locally on your device.')}</div>{/if}
  <footer class="scan-footer"><p>Local checks · no uploads. A clean result cannot guarantee safety.</p><div><button class="scan-secondary" onclick={onclose}>{running?'Run in background':'Done'}</button>{#if running}<button class="scan-secondary" onclick={oncancel}>Cancel scan</button>{:else if available}<button class="primary" disabled={disabled} onclick={onscan}>{report?'Scan again':'Start scan'}</button>{/if}</div></footer>
</section>

<style>
  .selection-scope{font-size:11px;font-weight:600;color:var(--text);padding:7px 10px;background:var(--soft);border:1px solid var(--line);border-radius:4px;margin-bottom:16px}.selection-scope span{font-weight:400;color:var(--muted)}
  .scan-panel{--result-color:var(--muted);--result-tint:var(--soft);--green:#257248;--amber:#9b650d;--red:#b43832;font-size:13px;line-height:1.5}
  .scan-panel[data-result=clear]{--result-color:var(--green);--result-tint:#edf7f0}
  .scan-panel[data-result=attention]{--result-color:var(--amber);--result-tint:#fff6e5}
  .scan-panel[data-result=blocked]{--result-color:var(--red);--result-tint:#fcefed}
  .scan-panel[data-result=running]{--result-color:var(--accent)}
  .result-hero{display:flex;align-items:center;gap:18px;padding:2px 0 18px}
  .result-emblem{width:64px;height:72px;color:var(--result-color);flex-shrink:0}
  .result-emblem svg{width:100%;height:100%}
  .result-heading{min-width:0}.result-heading h3{font-size:25px!important;letter-spacing:-.5px;margin:0 0 5px!important;line-height:1.2;color:var(--result-color)}
  .archive-name{font-size:12px;color:var(--muted);margin:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .status-scale{display:flex;gap:5px;margin:0 0 20px}.scale-segment{flex:1;min-width:0;display:flex;flex-direction:column;gap:6px;text-align:center;color:var(--muted)}
  .scale-pointer{font-size:10px;line-height:10px;height:10px;color:var(--result-color)}.scale-track{height:7px;border-radius:2px;background:var(--line)}
  .clear .scale-track{background:#d7e9dc}.attention .scale-track{background:#f3e5c8}.blocked .scale-track{background:#eed6d5}
  .clear.current .scale-track{background:var(--green)}.attention.current .scale-track{background:var(--amber)}.blocked.current .scale-track{background:var(--red)}
  .scale-label{font-size:10px}.current .scale-label{color:var(--result-color);font-weight:650}
  .result-story{border-left:3px solid var(--result-color);padding:11px 14px;background:var(--result-tint);border-radius:0 4px 4px 0}.result-story strong{font-weight:550;font-size:13px}.result-story p{margin:3px 0 0;font-size:11px;color:var(--muted)}
  .coverage-line{display:flex;align-items:center;gap:7px;font-size:11px;color:var(--muted);padding:13px 0}.coverage-line>span:first-child{color:var(--green)}.coverage-line.partial>span:first-child{color:var(--muted)}.coverage-line small{margin-left:auto;font-size:10px}
  .issue-list{border-top:1px solid var(--line);border-bottom:1px solid var(--line);max-height:230px;overflow:auto;margin-bottom:10px}
  .issue-row+.issue-row{border-top:1px solid var(--line)}.issue-row summary{display:flex;align-items:center;gap:10px;padding:12px 0;list-style:none;cursor:pointer}.issue-row summary::-webkit-details-marker{display:none}
  .issue-symbol{height:23px;width:23px;flex-shrink:0;display:grid;place-items:center;border-radius:50%;font-size:13px;font-weight:650;color:var(--result-color);background:var(--result-tint)}
  .issue-copy{flex:1;min-width:0;display:flex;flex-direction:column;gap:3px}.issue-copy strong{font-size:12px;font-weight:550;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.issue-copy small{font-size:11px;color:var(--muted)}
  .issue-label{color:var(--result-color);font-size:10px;font-weight:600}.chevron{color:var(--muted)}.issue-row[open] .chevron{transform:rotate(180deg)}
  .issue-detail{padding:0 10px 12px 33px;color:var(--muted);font-size:12px}.issue-detail p{margin:0 0 8px}.issue-detail code{font-size:10px;overflow-wrap:anywhere}
  .scan-notes,.scan-technical{font-size:11px}.scan-notes>summary,.scan-technical>summary{cursor:pointer;padding:9px 0;font-weight:500;color:var(--muted)}.scan-notes>summary>span{margin-left:6px;font-size:10px}
  .scan-notes>div{padding:8px 0 8px 14px;border-left:2px solid var(--line);margin:5px 0;font-size:11px}.scan-notes small{display:block;color:var(--muted)}.scan-notes p{color:var(--muted);margin:4px 0}
  .scan-technical{max-height:220px;overflow:auto}.scan-technical>summary>span{float:right;font-size:10px;font-weight:400}
  dl{margin:12px 0}dl>div{display:grid;grid-template-columns:112px 1fr;gap:12px;padding:6px 0}dt{color:var(--muted)}dd{margin:0;overflow-wrap:anywhere}dd code{font-size:10px}.scan-technical p{color:var(--muted);overflow-wrap:anywhere}
  .scan-footer{margin-top:14px;border-top:1px solid var(--line);padding-top:12px;background:var(--surface)}.scan-footer>p{font-size:10px;color:var(--muted);margin:0 0 12px}.scan-footer>div{display:flex;justify-content:flex-end;gap:8px}.scan-secondary{padding:8px 13px;border:1px solid var(--line);background:var(--surface);border-radius:5px;font-size:12px}
  .scan-live{padding:4px 0 20px}.live-label{display:flex;justify-content:space-between;margin-bottom:10px;font-size:12px}.live-label strong{font-variant-numeric:tabular-nums}
  progress{width:100%;height:6px;accent-color:var(--accent);display:block;border:0;border-radius:3px;overflow:hidden}progress::-webkit-progress-bar{background:var(--line)}progress::-webkit-progress-value{background:var(--accent)}
  .scan-live p{font-size:11px;color:var(--muted);margin:10px 0 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.scan-empty{padding:14px 0;color:var(--muted);font-size:12px}
  .working{animation:pulse 1.5s ease-in-out infinite}@keyframes pulse{50%{opacity:.45}}@media(prefers-reduced-motion:reduce){.working{animation:none}}
  :global(.application[data-theme=dark]) .scan-panel{--green:#79c99b;--amber:#e4b352;--red:#f19b94}
  :global(.application[data-theme=dark]) .scan-panel[data-result=clear]{--result-tint:#26382d}:global(.application[data-theme=dark]) .scan-panel[data-result=attention]{--result-tint:#3b3222}:global(.application[data-theme=dark]) .scan-panel[data-result=blocked]{--result-tint:#402b2a}
  @media(prefers-color-scheme:dark){:global(.application[data-theme=system]) .scan-panel{--green:#79c99b;--amber:#e4b352;--red:#f19b94}:global(.application[data-theme=system]) .scan-panel[data-result=clear]{--result-tint:#26382d}:global(.application[data-theme=system]) .scan-panel[data-result=attention]{--result-tint:#3b3222}:global(.application[data-theme=system]) .scan-panel[data-result=blocked]{--result-tint:#402b2a}}
  @media(max-width:480px){.result-heading h3{font-size:21px!important}.result-emblem{width:52px;height:60px}.result-hero{gap:14px}.scan-technical>summary>span{display:none}}
</style>
