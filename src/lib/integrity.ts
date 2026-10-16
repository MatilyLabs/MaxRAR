export interface TestReceipt { operation:string; written:string[]; skipped:string[]; bytes_processed:number; message:string; }
export type TestState='ready'|'running'|'passed'|'failed'|'password'|'cancelled'|'incomplete';
export function testOutcome(receipt:TestReceipt|null,failure:string,encrypted=false):{state:TestState;title:string;reason:string}{
 if(failure){
  if(/cancel/i.test(failure))return {state:'cancelled',title:'Test cancelled',reason:'The archive was not fully checked.'};
  if(/password|decrypt|authentication/i.test(failure))return {state:'password',title:'Password required',reason:'Enter the correct password to continue.'};
  if(/permission|denied|limit|exceed|unsupported|archive changed|no such file|not found/i.test(failure))return {state:'incomplete',title:'Couldn’t finish',reason:'The archive was not fully checked.'};
  if(/checksum|crc32|crc (?:check|mismatch|error)|corrupt|sha-?256 mismatch|hash mismatch|digest mismatch/i.test(failure))return encrypted?{state:'incomplete',title:'Password or data error',reason:'Check the password, then test again.'}:{state:'failed',title:'Integrity check failed',reason:'Corruption was detected. The test stopped at the first error.'};
  return {state:'incomplete',title:'Couldn’t finish',reason:'The archive was not fully checked.'};
 }
 if(receipt){
  if(receipt.skipped.length)return {state:'incomplete',title:'Some files unchecked',reason:`${receipt.skipped.length} items were skipped.`};
  return {state:'passed',title:'Integrity passed',reason:'No errors detected by the supported checks.'};
 }
 return {state:'ready',title:'Ready to test',reason:'Checks readability and available checksums.'};
}
