'use strict';
(() => {
 const $=id=>document.getElementById(id);let packages=[],busy=false;
 const secure=()=>location.protocol==='https:'||['127.0.0.1','localhost','[::1]'].includes(location.hostname);
 async function api(path,options={}) {
  if(!secure())throw new Error('Gunakan HTTPS atau loopback lokal, bukan file://.');
  if(!$('mlToken').value)throw new Error('Token MLOPS admin diperlukan.');
  const response=await fetch('/api/ml'+path,{...options,headers:{...options.headers,Authorization:`Bearer ${$('mlToken').value}`},cache:'no-store',signal:AbortSignal.timeout(60000)});
  if(!response.ok)throw new Error(`HTTP ${response.status}: ${await response.text()}`);
  return response;
 }
 async function action(work) {
  if(busy)return;busy=true;
  ['mlValidate','mlUpload','mlRefresh','mlCompare','mlDownload','mlPdf','mlRetrain'].forEach(id=>{if($(id))$(id).disabled=true;});
  try{await work();}catch(e){$('mlStatus').textContent=e.message;}
  finally{busy=false;['mlValidate','mlUpload','mlRefresh','mlCompare','mlRetrain'].forEach(id=>{if($(id))$(id).disabled=false;});$('mlDownload').disabled=$('mlPdf').disabled=!$('mlSelect').value;}
 }
 function display(record) {
  const box=$('mlResults');box.replaceChildren();
  if(!record){box.textContent='Accuracy / macro-F1 / confusion matrix belum tersedia.';return;}
  const meta=document.createElement('pre');meta.className='ml-output';
  meta.textContent=JSON.stringify({...record,metrics:undefined},null,2);box.append(meta);
  if(!record.metrics){const p=document.createElement('p');p.textContent='Accuracy / macro-F1 / confusion matrix unavailable: belum ada prediksi evaluasi terikat fixed test. V1 tidak diberi skor buatan.';box.append(p);return;}
  for(const split of ['validation','test']) {
   const title=document.createElement('h4');title.textContent=`${split.toUpperCase()} · ${record.purpose}`;box.append(title);
   const table=document.createElement('table');const head=document.createElement('tr');
   for(const label of ['Level','Accuracy','Macro-F1','Support']){const th=document.createElement('th');th.textContent=label;head.append(th);}table.append(head);
   for(const m of record.metrics[split]){
    const tr=document.createElement('tr');for(const value of [`L${m.level}`,m.accuracy.toFixed(4),m.macro_f1.toFixed(4),m.support]){const td=document.createElement('td');td.textContent=value;tr.append(td);}table.append(tr);
   }box.append(table);
   for(const m of record.metrics[split]){const pre=document.createElement('pre');pre.className='ml-output';pre.textContent=`L${m.level} confusion matrix\nUrutan label: ${JSON.stringify(m.labels)}\nBaris = truth; kolom = prediction\n${m.confusion_matrix.map(r=>r.join('  ')).join('\n')}`;box.append(pre);}
  }
 }
 async function refresh(selected=''){
  const data=await (await api('/packages')).json();packages=data.packages;
  for(const id of ['mlSelect','mlLeft','mlRight','mlRetrainTarget']){
   const select=$(id);if(!select)continue;const old=selected||select.value;select.replaceChildren(new Option('Pilih paket',''));
   for(const p of packages)select.add(new Option(`${p.experiment?.name||'Dataset saja'} · ${p.purpose} · ${p.id}`,p.id));
   if(packages.some(p=>p.id===old))select.value=old;
  }
  display(packages.find(p=>p.id===$('mlSelect').value));$('mlStatus').textContent=`Registry terbaca: ${data.count} paket. Namespace real / synthetic_test terpisah.`;
 }
 async function payload(){const file=$('mlFile').files[0];if(!file)throw new Error('Pilih package JSON.');if(file.size>32*1024*1024)throw new Error('Package melebihi 32 MiB.');return await file.text();}
 $('mlValidate').onclick=()=>action(async()=>{const data=await(await api('/validate',{method:'POST',headers:{'Content-Type':'application/json'},body:await payload()})).json();display(data);$('mlStatus').textContent='Validasi server lulus. Belum disimpan; attestation label belum diverifikasi independen.';});
 $('mlUpload').onclick=()=>action(async()=>{const data=await(await api('/packages',{method:'POST',headers:{'Content-Type':'application/json'},body:await payload()})).json();const check=await(await api('/packages/'+data.id)).json();if(check.package_sha256!==data.package_sha256)throw new Error('Readback package berbeda.');await refresh(data.id);$('mlStatus').textContent=`Import + readback lulus: ${data.id}; ${data.purpose}. Tidak ada training remote atau flashing.`;});
 if($('mlRetrain')) $('mlRetrain').onclick=()=>action(async()=>{const id=$('mlRetrainTarget').value;if(!id)throw new Error('Pilih paket.');const data=await(await api('/retrain',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({package_id:id})})).json();await refresh(data.id);$('mlStatus').textContent=`Retrain selesai: ${data.id}.`;});
 $('mlRefresh').onclick=()=>action(()=>refresh());
 $('mlSelect').onchange=()=>{display(packages.find(p=>p.id===$('mlSelect').value));$('mlDownload').disabled=$('mlPdf').disabled=!$('mlSelect').value;};
 for(const [button,suffix,name] of [['mlDownload','download','ml-package.json'],['mlPdf','report','experiment-report.pdf']]){
  $(button).onclick=()=>action(async()=>{const id=$('mlSelect').value;if(!id)throw new Error('Pilih paket.');const blob=await(await api(`/packages/${encodeURIComponent(id)}/${suffix}`)).blob();const a=document.createElement('a');a.href=URL.createObjectURL(blob);a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(a.href),1000);$('mlStatus').textContent=`Unduh ${name} selesai; ${id}.`;});
 }
 $('mlCompare').onclick=()=>action(async()=>{const left=$('mlLeft').value,right=$('mlRight').value;$('mlComparison').textContent='Belum ada hasil perbandingan baru.';if(!left||!right)throw new Error('Pilih baseline dan kandidat.');try{const data=await(await api('/compare?'+new URLSearchParams({left,right}))).json();$('mlComparison').textContent=JSON.stringify(data,null,2);$('mlStatus').textContent='Comparable pada fixed test yang sama; delta kandidat minus baseline. Bukan bukti generalisasi atau validasi remote.';}catch(e){$('mlComparison').textContent=e.message;throw e;}});
 window.addEventListener('pagehide',()=>{$('mlToken').value='';});
})();
