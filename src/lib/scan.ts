export interface ScanFinding {code:string;severity:string;message:string;entry_id?:string;path?:string;}
export interface ScanReport {
  scope?:{kind:'archive'|'selection';entry_ids:string[];selected_files:number};
  status:string;message:string;archive_path:string;archive_identity:string;engine:string;engine_version:string;rule_version:string;
  structural_findings:ScanFinding[];rule_findings:ScanFinding[];
  coverage:{total_files:number;scanned_files:number;skipped_files:number;truncated_files:number;decoded_bytes:number;nested_archives:number;opaque_containers:number;complete:boolean};
  skipped:{entry_id:string;path:string;reason:string}[];elapsed_ms:number;
  limits:{max_entries:number;max_entry_bytes:number;max_total_bytes:number;max_duration_secs:number;max_depth:number};warnings:string[];
}

export function findingTitle(code:string):string {
  return ({WonRaR_Test_Sentinel:'Harmless scanner test',WonRaR_EICAR_Test_File:'Antivirus test file',WonRaR_PowerShell_Download_And_Execute:'Downloads and runs software',WonRaR_Shell_Download_Pipe_Execution:'Downloads and runs software',WonRaR_Python_Deserialization_With_Process_Call:'May run commands when loaded',unsafe_path:'Unsafe extraction path',special_entry:'Blocked link or special file',duplicate_path:'Conflicting filenames',executable_content:'Contains a program or script',high_expansion:'Expands to an unusually large size',encrypted:'Password-protected content',double_extension:'Disguised executable filename',extension_magic_mismatch:'File type does not match its name',pickle_serialization:'May run code when loaded'} as Record<string,string>)[code] || code.replace(/^WonRaR_/,'').replaceAll('_',' ');
}

export function scanOutcome(report:ScanReport) {
  const findings=[...report.structural_findings,...report.rule_findings];
  const issues=findings.filter(f=>!['test','info'].includes(f.severity));
  const blocked=issues.some(f=>['error','critical','danger'].includes(f.severity));
  const complete=report.coverage.complete && report.coverage.total_files>0 && report.coverage.scanned_files===report.coverage.total_files && report.coverage.skipped_files===0 && report.coverage.truncated_files===0 && report.coverage.opaque_containers===0;
  const state=blocked?'blocked':issues.length?'attention':complete?'clear':'incomplete';
  return {
    state,
    title:state==='blocked'?'Unsafe entries detected':state==='attention'?'Needs attention':state==='clear'?'No issues detected':'Scan incomplete',
    label:state==='blocked'?'Unsafe entries':state==='attention'?'Caution':state==='clear'?'No issues detected':'Unknown',
    reason:state==='blocked'?'Unsafe archive entries need review before extraction.':state==='attention'?issues.length===1?(['WonRaR_PowerShell_Download_And_Execute','WonRaR_Shell_Download_Pipe_Execution'].includes(issues[0].code)?'A file contains signs of a download-and-run command.':findingTitle(issues[0].code)+'.'):`${issues.length} items need your review.`:state==='clear'?'No warning signs found by the current checks.':'Some content could not be fully checked.',
    complete,issues,
    notes:findings.filter(f=>['test','info'].includes(f.severity))
  };
}
