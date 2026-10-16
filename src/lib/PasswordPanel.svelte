<script lang="ts">
 let { oncontinue, oncancel }: { oncontinue:(password:string)=>void; oncancel:()=>void }=$props();
 let secret=$state('');
 function submit(event:SubmitEvent){event.preventDefault();if(!secret)return;const value=secret;secret='';oncontinue(value);}
 function cancel(){secret='';oncancel();}
</script>
<form onsubmit={submit}>
 <p class="password-hint">Enter the password to access protected contents.</p>
 <label for="archive-password">Archive password</label>
 <input id="archive-password" type="password" bind:value={secret} autocomplete="off"/>
 <div class="dialog-actions"><button type="button" onclick={cancel}>Cancel</button><button class="primary" type="submit" disabled={!secret}>Continue</button></div>
</form>
<style>.password-hint{font-size:12px;color:var(--muted);margin:0 0 18px;line-height:1.5}label{display:block;font-size:12px;font-weight:550;margin-bottom:8px}input{width:100%;font-size:13px}</style>
