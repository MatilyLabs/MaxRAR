import type {Entry} from './bridge';
export interface ContextAction {id:string;label:string;icon:string;disabled?:boolean;reason?:string;separator?:boolean;danger?:boolean;}
export function directChild(entry:Entry,folder:string){
  const name=entry.name.replace(/\/$/,'');
  if(!folder)return !name.includes('/');
  if(!name.startsWith(folder+'/'))return false;
  const rest=name.slice(folder.length+1);
  return !!rest&&!rest.includes('/');
}
export function selectedArchiveIds(entries:Entry[],selection:Entry[]){
  const ids=new Set(selection.map(e=>e.id)),folders=new Set(selection.filter(e=>e.is_dir).map(e=>e.name.replace(/\/$/,'')));
  return entries.filter(entry=>{
    if(ids.has(entry.id))return true;
    if(!folders.size)return false;
    let name=entry.name.replace(/\/$/,'');
    for(let slash=name.lastIndexOf('/');slash>=0;slash=name.lastIndexOf('/')){name=name.slice(0,slash);if(folders.has(name))return true;}
    return false;
  }).map(e=>e.id);
}
export function scanEntryIds(entries:Entry[],selection:Entry[]){
  const ids=new Set(selectedArchiveIds(entries,selection));
  return entries.filter(e=>!e.is_dir&&ids.has(e.id)).map(e=>e.id);
}
export function contextActions(selection:Entry[],options:{archive:boolean;canExtract:boolean;canDelete:boolean;canScan:boolean;canCreate:boolean;busy:boolean}):ContextAction[]{
  const {archive,canExtract,canDelete,canScan,canCreate,busy}=options;
  if(!selection.length)return [
    {id:'openArchive',label:'Open archive…',icon:'archive',disabled:busy},
    {id:'add',label:'Create new archive…',icon:'add',disabled:busy||!canCreate},
    {id:'selectAll',label:'Select all',icon:'document',separator:true,disabled:!archive},
    {id:'extractAll',label:'Extract archive…',icon:'extract',disabled:busy||!canExtract},
    {id:'scanAll',label:'Scan archive',icon:'scan',disabled:busy||!canScan},
    {id:'copyArchive',label:'Copy archive path',icon:'document',separator:true,disabled:!archive},
    {id:'archiveInfo',label:'Archive information',icon:'info',disabled:!archive}
  ];
  const single=selection.length===1,dir=single&&selection[0].is_dir;
  const actions:ContextAction[]=[];
  if(single)actions.push({id:dir?'openFolder':'preview',label:dir?'Open folder':'Preview',icon:dir?'folder':'view',disabled:!dir&&busy});
  actions.push(
    {id:'extract',label:dir?'Extract folder…':single?'Extract file…':`Extract ${selection.length} selected items…`,icon:'extract',disabled:busy||!canExtract},
    {id:'scanSelection',label:dir?'Scan folder':single?'Scan selected file':`Scan ${selection.length} selected items`,icon:'scan',disabled:busy||!canScan},
    {id:'copyNames',label:single?'Copy filename':'Copy filenames',icon:'document',separator:true},
    {id:'copyPaths',label:single?'Copy path in archive':'Copy paths in archive',icon:'document'},
    {id:'entryInfo',label:dir?'Folder information':single?'File information':'Selection information',icon:'info',separator:true},
    {id:'delete',label:'Delete from archive…',icon:'delete',danger:true,separator:true,disabled:busy||!canDelete,reason:!canDelete?'Entry deletion is unavailable for this archive format.':undefined}
  );
  return actions;
}
