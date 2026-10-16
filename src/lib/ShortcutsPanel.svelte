<script lang="ts">
 import {commandShortcuts} from './shortcuts';
 let {onclose,apple=false}:{onclose:()=>void;apple?:boolean}=$props();
</script>
<section class="shortcuts-panel" aria-label="Keyboard shortcuts">
 <h3>Archive</h3><dl>{#each commandShortcuts as shortcut}<div><dt>{shortcut.label}</dt><dd><kbd>{apple?'⌘':'Ctrl'}</kbd><kbd>{shortcut.key.toUpperCase()}</kbd></dd></div>{/each}</dl>
 <h3>File list</h3><dl>{#each [{label:'Open folder or preview file',key:'Enter'},{label:'Move between files',key:'↑ / ↓'},{label:'First or last file',key:'Home / End'},{label:'Extend selection',key:'Shift + ↑ / ↓'},{label:'Open context menu',key:'Shift + F10'}] as shortcut}<div><dt>{shortcut.label}</dt><dd><kbd>{shortcut.key}</kbd></dd></div>{/each}</dl>
 <h3>Menus and dialogs</h3><dl><div><dt>Navigate menu items</dt><dd><kbd>↑ / ↓</kbd></dd></div><div><dt>Close menu or dialog</dt><dd><kbd>Esc</kbd></dd></div></dl>
 <footer><button class="primary" onclick={onclose}>Close</button></footer>
</section>
<style>.shortcuts-panel{font-size:13px;color:var(--text)}h3{margin:0 0 8px;font-size:12px;font-weight:550;color:var(--muted)}dl{margin:0 0 22px}dl>div{display:flex;align-items:center;justify-content:space-between;gap:20px;padding:8px 0;border-bottom:1px solid var(--line)}dt{font-size:12px}dd{display:flex;gap:4px;margin:0;flex-shrink:0}kbd{font-family:inherit;font-size:11px;line-height:1.5;border:1px solid var(--line);border-radius:4px;background:var(--soft);padding:2px 6px;min-width:22px;text-align:center}footer{display:flex;justify-content:flex-end;border-top:1px solid var(--line);padding-top:16px}button{font-size:13px;padding:8px 18px;border-radius:6px;min-width:88px}</style>
