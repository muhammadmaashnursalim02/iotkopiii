"""Dedicated loopback test runner. Private temp token/storage; no activation or DB writes."""
import os, secrets, subprocess, tempfile, time, sys
from pathlib import Path
import requests
ROOT=Path(__file__).resolve().parents[2]
private=Path(tempfile.mkdtemp(prefix='coffee-ota-test-'))
if os.name=='nt':
    subprocess.run(['icacls',str(private),'/inheritance:r','/grant:r',os.environ['USERNAME']+':(OI)(CI)F'],check=True,capture_output=True)
else: private.chmod(0o700)
tokenfile=private/'admin-token.txt'; tokenfile.write_text(secrets.token_urlsafe(48))
env=os.environ.copy(); env.update(PORT='3001',OTA_STORAGE_DIR=str(private/'artifacts'),OTA_ADMIN_TOKEN=tokenfile.read_text(),OTA_TEST_TOKEN_FILE=str(tokenfile))
log=(private/'runner.log').open('w')
process=subprocess.Popen([str(ROOT/'target/debug/dashboard_readonly.exe')],cwd=ROOT,env=env,stdout=log,stderr=log)
try:
    for _ in range(60):
        if process.poll() is not None: raise RuntimeError('Runner exited; inspect '+str(private/'runner.log'))
        try:
            if requests.get('http://127.0.0.1:3001/api/dashboard?limit=1',timeout=2).status_code==200:break
        except requests.RequestException:pass
        time.sleep(0.5)
    else:raise RuntimeError('Runner readiness timeout')
    print('Dedicated TEST ONLY storage:',private,'PID:',process.pid,flush=True)
    result=subprocess.run([sys.executable,str(ROOT/'dashboard/tests/test_report_ota.py')],cwd=ROOT,env=env)
    raise SystemExit(result.returncode)
finally:
    process.terminate(); process.wait(timeout=10); log.close()
