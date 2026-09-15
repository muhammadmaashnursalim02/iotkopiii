"""Offline labeled ML workflow. Imported models remain opaque; never pickle.load.
Rust validator owns schema/fingerprints to avoid cross-language float hash drift.
"""
import argparse, csv, hashlib, json, os, pickle, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
MAX=32*1024*1024

def load(path):
    path=Path(path)
    if path.stat().st_size>MAX: raise ValueError('Input exceeds 32 MiB')
    return json.loads(path.read_text(encoding='utf-8'))

def write(path,value):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('x',encoding='utf-8') as f: json.dump(value,f,ensure_ascii=False,allow_nan=False,indent=2)

def package(d): return dict(schema_version=1,dataset=d,model=None,experiment=None)

def validate(p,validator):
    r=subprocess.run([str(Path(validator).resolve())],input=json.dumps(p,allow_nan=False).encode(),capture_output=True,timeout=60)
    if r.returncode: raise ValueError(r.stderr.decode('utf-8',errors='replace'))
    return json.loads(r.stdout)

def fixture():
    import numpy as np
    rng=np.random.default_rng(1909)
    d=dict(schema_version=1,purpose='synthetic_test',label_provenance=dict(kind='synthetic_test',verified_by='seeded fixture generator',evidence='TEST ONLY; no coffee performance'),group_ids=[f'G{i:02}' for i in range(18)],feature_names=['sensor_a','sensor_b'],feature_units=['V','V'],interval_ms=10,samples=[])
    for g in range(18):
        for c in range(4):
            values=rng.normal([c*2,(c%2)*3],0.3,(12,2)).round(6).tolist()
            d['samples'].append(dict(sample_id=f'G{g:02}-S{c}',session_id=f'G{g:02}-session',group_id=f'G{g:02}',split='train' if g<12 else 'validation' if g<15 else 'test',labels=[f'species{c//2}',f'roast{c%2}',f'class{c}'],values=values))
    return d

def features(samples):
    import numpy as np
    # Per-sample fixed transform, no fitting and no across-sample aggregation.
    return np.asarray([np.concatenate([np.mean(s['values'],axis=0),np.std(s['values'],axis=0)]) for s in samples])

def train(d,out,algorithm,name,validator):
    import numpy as np
    import sklearn
    from sklearn.pipeline import make_pipeline
    from sklearn.preprocessing import StandardScaler
    from sklearn.svm import SVC
    from sklearn.discriminant_analysis import LinearDiscriminantAnalysis
    from sklearn.metrics import accuracy_score,f1_score,confusion_matrix
    summary=validate(package(d),validator)
    train_rows=sorted([s for s in d['samples'] if s['split']=='train'],key=lambda s:s['sample_id'])
    held=sorted([s for s in d['samples'] if s['split']!='train'],key=lambda s:s['sample_id'])
    x=features(train_rows); xt=features(held);models=[];columns=[]
    for level in range(3):
        y=[s['labels'][level] for s in train_rows]
        if len(set(y))<2: raise ValueError(f'L{level+1}: at least two training classes required')
        estimator=SVC(C=1.0,kernel='rbf',gamma='scale',random_state=1909) if algorithm=='svm' else LinearDiscriminantAnalysis(solver='lsqr',shrinkage='auto')
        model=make_pipeline(StandardScaler(),estimator);model.fit(x,y)
        models.append(model);columns.append(model.predict(xt))
    predictions=[dict(sample_id=s['sample_id'],labels=[str(columns[l][i]) for l in range(3)]) for i,s in enumerate(held)]
    # Only serialize models created in this process. Never deserialize uploaded models.
    raw=pickle.dumps(dict(models=models,features='per-window mean,std; channel order preserved',feature_names=d['feature_names'],sklearn_version=sklearn.__version__),protocol=5)
    p=package(d);p['model']=dict(format='sklearn_pickle_opaque',bytes_hex=raw.hex())
    p['experiment']=dict(name=name,engine='python_'+algorithm,model_sha256=hashlib.sha256(raw).hexdigest(),dataset_fingerprint=summary['dataset_fingerprint'],test_fingerprint=summary['test_fingerprint'],evaluation_protocol='heldout_group_v1',preprocessing='Per-window mean/std; StandardScaler fit on train only; fixed defaults, no test tuning; independent L1/L2/L3 classifiers',provenance=f'Local sklearn {sklearn.__version__}; seed 1909; {d["purpose"]}; no remote Edge Impulse execution',predictions=predictions)
    result=validate(p,validator)
    for split in ['validation','test']:
        ix=[i for i,s in enumerate(held) if s['split']==split]
        for l in range(3):
            truth=[held[i]['labels'][l] for i in ix];pred=[predictions[i]['labels'][l] for i in ix];m=result['metrics'][split][l]
            assert np.isclose(m['accuracy'],accuracy_score(truth,pred))
            assert np.isclose(m['macro_f1'],f1_score(truth,pred,average='macro',zero_division=0))
            assert m['confusion_matrix']==confusion_matrix(truth,pred,labels=m['labels']).tolist()
    out=Path(out);out.mkdir(parents=True,exist_ok=False)
    (out/'model.pkl').write_bytes(raw);write(out/'package.json',p);write(out/'summary.json',result)
    write(out/'run.json',dict(python=sys.executable,python_version=sys.version,sklearn_version=sklearn.__version__,algorithm=algorithm,seed=1909,fit_sample_ids=[s['sample_id'] for s in train_rows],scaler_means=[m[0].mean_.tolist() for m in models],hierarchy_policy='Independent heads; inconsistent parent predictions possible and not silently repaired',deployment='Not ESP32 firmware; conversion and board build are separate compatibility gates'))
    return result

def export_ei(d,out,validator):
    summary=validate(package(d),validator);out=Path(out);out.mkdir(parents=True,exist_ok=False)
    for l in range(3):
        level=out/f'level{l+1}';level.mkdir()
        for split in ['train','validation','test']:
            files=[]
            for s in sorted(d['samples'],key=lambda s:s['sample_id']):
                if s['split']!=split:continue
                # Safe generated index names, never label-controlled paths.
                name=f'sample{len(files):05}-{split}.csv'
                with (level/name).open('x',newline='',encoding='utf-8') as f:
                    w=csv.writer(f);w.writerow(['timestamp',*d['feature_names']]);w.writerows([[i*d['interval_ms'],*r] for i,r in enumerate(s['values'])])
                files.append(dict(path=name,category='training' if split=='train' else 'testing',label=dict(type='label',label=s['labels'][l]),metadata=dict(sample_id=s['sample_id'],session_id=s['session_id'],group_id=s['group_id'],local_split=split,purpose=d['purpose'])))
            write(level/f'{split}.labels',dict(version=1,files=files))
    write(out/'manifest.json',dict(**summary,remote_status='unverified; export only; no network operation',source_dataset=d,format='Edge Impulse CSV + uploader info-file version 1',validation_policy='Validation exported separately as testing. Keep separate EI projects for validation and final test, never merge or auto-split; external training isolation must be audited. One project per level.'))
    return summary

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--validator',default=str(ROOT/'target/debug'/('ml_validate.exe' if os.name=='nt' else 'ml_validate')))
    sub=ap.add_subparsers(dest='command',required=True)
    f=sub.add_parser('fixture');f.add_argument('--out',required=True)
    for command in ['train','export-ei','validate','pack']:
        p=sub.add_parser(command);p.add_argument('--dataset',required=True)
        if command!='validate':p.add_argument('--out',required=True)
        if command=='train':p.add_argument('--algorithm',choices=['svm','lda'],default='svm');p.add_argument('--name',default='V2')
        if command=='pack':p.add_argument('--model');p.add_argument('--experiment');p.add_argument('--format',choices=['edge_impulse_opaque','sklearn_pickle_opaque','onnx_opaque'],default='edge_impulse_opaque')
    args=ap.parse_args()
    if args.command=='fixture':
        d=fixture();validate(package(d),args.validator);write(args.out,d);print('synthetic_test fixture only; no coffee performance');return
    d=load(args.dataset)
    if args.command=='train':r=train(d,args.out,args.algorithm,args.name,args.validator)
    elif args.command=='export-ei':r=export_ei(d,args.out,args.validator)
    else:
        p=package(d)
        if args.command=='pack':
            if args.model:
                raw=Path(args.model).read_bytes()
                if len(raw)>8*1024*1024:raise ValueError('Model exceeds 8 MiB')
                p['model']=dict(format=args.format,bytes_hex=raw.hex())
            if args.experiment:p['experiment']=load(args.experiment)
        r=validate(p,args.validator)
        if args.command=='pack':write(args.out,p)
    print(json.dumps(r,ensure_ascii=True))
if __name__=='__main__':
    try:main()
    except (ValueError,OSError,subprocess.SubprocessError) as e:print(str(e),file=sys.stderr);sys.exit(1)
