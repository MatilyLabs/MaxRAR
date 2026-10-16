import links from '../../release.config.json' with {type:'json'};
import metadata from '../../package.json' with {type:'json'};
export type ProjectLink='repository'|'manual'|'issues'|'organization'|'downloads';
export interface ReleaseInfo {version:string;configured:boolean;unavailableReason?:string|null;links:typeof links;}
export type UpdateStage='idle'|'unavailable'|'checking'|'current'|'available'|'downloading'|'verifying'|'ready'|'installing'|'error';
export interface UpdateView {stage:UpdateStage;version?:string;notes?:string;downloaded?:number;total?:number|null;error?:string;retry?:'check'|'download'|'install';}
export const previewRelease:ReleaseInfo={version:metadata.version,configured:false,unavailableReason:'not_configured',links};
export const updateBusy=(stage:UpdateStage)=>['checking','downloading','verifying','installing'].includes(stage);
export function shouldAutoCheck(enabled:boolean,configured:boolean,lastAttempt:number,now=Date.now()){
 return enabled&&configured&&(!Number.isFinite(lastAttempt)||lastAttempt<=0||lastAttempt>now||now-lastAttempt>=24*60*60*1000);
}
export function updateTitle(view:UpdateView,reason?:string|null){
 switch(view.stage){case 'idle':return 'Keep MaxRAR up to date';case 'unavailable':return reason==='package_manager'?'Update with a Linux package':'Updates unavailable';case 'checking':return 'Checking for updates…';case 'current':return 'You’re up to date';case 'available':return `MaxRAR ${view.version} is available`;case 'downloading':return 'Downloading update…';case 'verifying':return 'Verifying download…';case 'ready':return 'Update ready to install';case 'installing':return 'Installing update…';case 'error':return 'Update couldn’t be completed';}
}
