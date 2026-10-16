export interface CreateSource {path:string;type:'file'|'folder'}
export interface CreateDraft {inputs:string[];output:string;password:string|null}
export const createFormats=[
  {id:'zip',label:'ZIP',hint:'Most compatible',password:true,single:false},
  {id:'7z',label:'7z',hint:'Compact archives',password:true,single:false},
  {id:'tar.gz',label:'TAR.GZ',hint:'Portable package',password:false,single:false},
  {id:'tar',label:'TAR',hint:'Without compression',password:false,single:false},
  {id:'tar.bz2',label:'TAR.BZ2',hint:'Bzip2 package',password:false,single:false},
  {id:'tar.xz',label:'TAR.XZ',hint:'XZ package',password:false,single:false},
  {id:'tar.zst',label:'TAR.ZST',hint:'Zstandard package',password:false,single:false},
  {id:'gz',label:'GZ',hint:'One file',password:false,single:true},
  {id:'bz2',label:'BZ2',hint:'One file',password:false,single:true},
  {id:'xz',label:'XZ',hint:'One file',password:false,single:true},
  {id:'zst',label:'ZST',hint:'One file',password:false,single:true},
  {id:'mrar',label:'MRAR (Experimental)',hint:'Shared compression for small files',password:false,single:false}
];
export function baseName(path:string){return path.split(/[\\/]/).filter(Boolean).pop()||path;}
export function replaceArchiveSuffix(path:string,format:string){
  if(!path)return '';
  return path.replace(/\.(tar\.(gz|bz2|xz|zst)|zip|7z|mrar|tar|gz|bz2|xz|zst)$/i,'')+'.'+format;
}
export function suggestedArchivePath(sources:CreateSource[],format:string){
  if(!sources.length)return 'Archive.'+format;
  const first=sources[0];const split=Math.max(first.path.lastIndexOf('/'),first.path.lastIndexOf('\\'));
  let stem=sources.length===1?baseName(first.path):'Archive';
  if(!createFormats.find(f=>f.id===format)?.single&&first.type==='file'&&sources.length===1)stem=stem.replace(/\.[^.]+$/,'')||stem;
  const parent=first.path.slice(0,split+1);
  const candidate=parent+stem+'.'+format;
  return sources.some(s=>s.path===candidate)?parent+stem+' archive.'+format:candidate;
}
export function createProblem(sources:CreateSource[],format:string,output:string,protect:boolean,password:string,confirmation:string){
  const spec=createFormats.find(f=>f.id===format);
  if(!spec)return 'Choose a supported archive format.';
  if(!sources.length)return 'Add files or a folder to get started.';
  if(spec.single&&(sources.length!==1||sources[0].type!=='file'))return `${spec.label} compresses one file. Use TAR.${spec.label} to package these items.`;
  if(!output.trim())return 'Choose where to save the archive.';
  // The core chooses its encoder from the output suffix; never display a different format.
  const suffix=output.match(/\.(tar\.(?:gz|bz2|xz|zst)|zip|7z|mrar|tar|gz|bz2|xz|zst)$/i)?.[1].toLowerCase();
  if(suffix!==format)return `Save the archive with a .${format} extension.`;
  if(sources.some(s=>s.path===output))return 'Choose a save location different from the original file.';
  if(protect&&!spec.password)return 'Password protection is available for ZIP and 7z.';
  if(protect&&!password)return 'Enter a password.';
  if(protect&&password!==confirmation)return 'Passwords do not match.';
  return '';
}
