"""Synthetic fixtures only. Never coffee performance evidence."""
import unittest, tempfile, subprocess, sys, json, csv
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
CLI=ROOT/'ml/workflow.py'
VALIDATOR=ROOT/'target/debug/ml_validate.exe'
class Workflow(unittest.TestCase):
 def test_real_training_and_edge_export_roundtrip(self):
  with tempfile.TemporaryDirectory(prefix='ml-synthetic-test-') as t:
   t=Path(t)
   def run(*args):
    r=subprocess.run([sys.executable,str(CLI),'--validator',str(VALIDATOR),*map(str,args)],capture_output=True,text=True)
    self.assertEqual(r.returncode,0,r.stderr);return r
   run('fixture','--out',t/'dataset.json')
   d=json.loads((t/'dataset.json').read_text());self.assertEqual(d['purpose'],'synthetic_test');self.assertEqual(len(d['group_ids']),18)
   run('train','--dataset',t/'dataset.json','--out',t/'svm','--algorithm','svm','--name','V2-synthetic')
   summary=json.loads((t/'svm/summary.json').read_text());self.assertEqual(len(summary['metrics']['test']),3)
   self.assertTrue((t/'svm/model.pkl').stat().st_size>100)
   package=json.loads((t/'svm/package.json').read_text());self.assertEqual(len(package['experiment']['predictions']),24)
   run('export-ei','--dataset',t/'dataset.json','--out',t/'ei')
   info=json.loads((t/'ei/level1/train.labels').read_text());self.assertEqual(info['version'],1)
   entry=info['files'][0];self.assertEqual(entry['category'],'training')
   with (t/'ei/level1'/entry['path']).open() as f: rows=list(csv.reader(f))
   self.assertEqual(rows[0][0],'timestamp');self.assertEqual(float(rows[2][0])-float(rows[1][0]),d['interval_ms'])
   self.assertEqual(len(list((t/'ei').rglob('*.csv'))),len(d['samples'])*3)
   run('train','--dataset',t/'dataset.json','--out',t/'lda','--algorithm','lda','--name','V2-LDA-synthetic')
   lda=json.loads((t/'lda/summary.json').read_text());self.assertEqual(lda['test_fingerprint'],summary['test_fingerprint'])
   # Refit after held-out perturbation must not alter trained weights/scaler.
   for s in d['samples']:
    if s['split']!='train': s['values']=[[v+1000 for v in row] for row in s['values']]
   (t/'changed.json').write_text(json.dumps(d))
   run('train','--dataset',t/'changed.json','--out',t/'changed','--algorithm','svm','--name','V2-synthetic')
   self.assertEqual((t/'changed/model.pkl').read_bytes(),(t/'svm/model.pkl').read_bytes())
   d['label_provenance']['kind']='pred_level_1';(t/'bad.json').write_text(json.dumps(d))
   bad=subprocess.run([sys.executable,str(CLI),'--validator',str(VALIDATOR),'train','--dataset',str(t/'bad.json'),'--out',str(t/'bad')],capture_output=True,text=True)
   self.assertNotEqual(bad.returncode,0);self.assertFalse((t/'bad/model.pkl').exists())
if __name__=='__main__':unittest.main()
