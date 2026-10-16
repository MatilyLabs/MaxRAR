/** Inventory-only descriptions. Names are hints, never proof of purpose or safety. */
import type { Entry } from './bridge';

export interface FileGroup {id:string;label:string;count:number;color:string;icon:string;}
const definitions:Record<string,{label:string;color:string;icon:string}>={
  html:{label:'HTML pages',color:'#458bda',icon:'document'},
  css:{label:'CSS styles',color:'#8971d2',icon:'code'},
  js:{label:'JavaScript',color:'#cd962c',icon:'code'},
  tex:{label:'TeX source',color:'#458bda',icon:'document'},
  bib:{label:'Bibliographies',color:'#8971d2',icon:'document'},
  images:{label:'Images',color:'#369e8b',icon:'document'},
  documents:{label:'Documents',color:'#647caa',icon:'document'},
  source:{label:'Source code',color:'#7a79b5',icon:'code'},
  data:{label:'Data files',color:'#599aa6',icon:'document'},
  archives:{label:'Archives',color:'#b59246',icon:'archive'},
  media:{label:'Audio & video',color:'#a979a0',icon:'document'},
  other:{label:'Other files',color:'#87909e',icon:'document'}
};
function category(entry:Entry,texIds:Set<string>){
  const name=entry.name.toLowerCase();
  const ext=name.split('/').pop()?.split('.').pop()||'';
  if(texIds.has(entry.id)||ext==='tex')return 'tex';
  if(['html','htm'].includes(ext))return 'html';
  if(['css','scss','sass','less'].includes(ext))return 'css';
  if(['js','jsx','mjs','cjs'].includes(ext))return 'js';
  if(['bib','bbl'].includes(ext))return 'bib';
  if(['png','jpg','jpeg','gif','svg','webp','avif','bmp','tif','tiff','eps'].includes(ext))return 'images';
  if(['pdf','doc','docx','odt','txt','md','rst','rtf','ppt','pptx','xls','xlsx'].includes(ext))return 'documents';
  if(['rs','py','ts','tsx','c','h','cpp','hpp','go','java','swift','kt','rb','php','sh','ps1','bat','cmd','sty','cls'].includes(ext))return 'source';
  if(['json','yaml','yml','toml','xml','csv','tsv','sql','parquet','npy','npz'].includes(ext))return 'data';
  if(['zip','7z','rar','mrar','tar','gz','bz2','xz','zst','zstd','tgz','whl','jar'].includes(ext))return 'archives';
  if(['mp3','wav','flac','ogg','m4a','mp4','mov','mkv','webm','avi'].includes(ext))return 'media';
  return 'other';
}
function mainScore(entry:Entry,paper:boolean,web:boolean,texIds:Set<string>){
  const base=entry.name.toLowerCase().split('/').pop()||'';
  if(web&&/^index\.html?$/.test(base))return 100;
  if(paper&&/^(main|paper|manuscript|article)\.tex$/.test(base))return 100;
  if(paper&&(texIds.has(entry.id)||base.endsWith('.tex')))return 70;
  if(/^readme(?:\.(md|txt|rst))?$/.test(base))return 85;
  if(['package.json','cargo.toml','pyproject.toml','index.html','index.htm'].includes(base))return 65;
  return 0;
}
export function archiveOverview(entries:Entry[],sourceHintIds:string[]=[]){
  const files=entries.filter(e=>!e.is_dir);
  const hints=new Set(sourceHintIds);
  const counts:Record<string,number>={};
  const folderMembers=new Map<string,{count:number;types:Record<string,number>}>();
  for(const e of files){
    const key=category(e,hints);counts[key]=(counts[key]||0)+1;
    if(e.name.includes('/')){
      const root=e.name.slice(0,e.name.indexOf('/'));
      const info=folderMembers.get(root)||{count:0,types:{}};
      info.count++;info.types[key]=(info.types[key]||0)+1;folderMembers.set(root,info);
    }
  }
  const paper=!!counts.tex,web=!!counts.html;
  if(paper){
    // PDFs are only figure candidates when TeX sources coexist.
    const pdfs=files.filter(e=>/\.pdf$/i.test(e.name)).length;
    if(pdfs){counts.images=(counts.images||0)+pdfs;counts.documents-=pdfs;}
  }
  const ordered=Object.keys(definitions).filter(id=>counts[id]>0);
  const groups:FileGroup[]=ordered.map(id=>({id,...definitions[id],label:id==='images'&&paper?'Possible figures':definitions[id].label,count:counts[id]}));
  if(groups.length>5){const rest=groups.splice(4);groups.push({id:'other',...definitions.other,count:rest.reduce((n,g)=>n+g.count,0)});}
  const folders=new Set<string>();
  for(const e of entries){const parts=e.name.replace(/\/$/,'').split('/');for(let i=1;i<parts.length+(e.is_dir?1:0);i++)folders.add(parts.slice(0,i).join('/'));}
  const roots=[...folders].filter(p=>!p.includes('/')).sort((a,b)=>a.localeCompare(b));
  const ranked=files.map(entry=>({entry,score:mainScore(entry,paper,web,hints)})).filter(e=>e.score>0).sort((a,b)=>b.score-a.score||a.entry.name.split('/').length-b.entry.name.split('/').length||a.entry.name.localeCompare(b.entry.name));
  let summary='A collection of files and folders.';
  if(!files.length)summary=folders.size?'Contains folders, with no files listed.':'This archive is empty.';
  else if(paper)summary='Likely research sources with TeX'+(counts.bib?', references':'')+((counts.images||0)?' and possible figures.':'.');
  else if(web)summary='Looks like a website with HTML pages'+(counts.css?', styles':'')+(counts.js?' and scripts.':'.');
  else if(files.some(e=>/(^|\/)(package\.json|cargo\.toml|pyproject\.toml)$/i.test(e.name)))summary=counts.source||counts.js?'Looks like a software project with source and configuration files.':'Contains project configuration files.';
  else if((counts.images||0)===files.length)summary='A collection of images.';
  else if((counts.documents||0)===files.length)summary='A collection of documents.';
  else if((counts.media||0)===files.length)summary='A collection of audio or video files.';
  else if((counts.archives||0)===files.length)summary='Contains other archives.';
  else if(counts.source)summary='Contains source code and supporting files.';
  const folderRows=roots.map(path=>{const info=folderMembers.get(path);const local=info?.types||{};const labels=Object.keys(local).sort((a,b)=>local[b]-local[a]).slice(0,2).map(id=>definitions[id].label.toLowerCase());return {path,count:info?.count||0,description:labels.join(' · ')||'No files listed'};});
  return {files:files.length,folders:folders.size,groups,summary,main:ranked[0]?.entry??null,folderRows,basis:sourceHintIds.length?'Based on filenames and source hints':'Inferred from filenames and folders'};
}
