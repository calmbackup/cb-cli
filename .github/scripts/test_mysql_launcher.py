"""Offline launcher tests: every Docker/MySQL/Cargo command is a synthetic stub.

These validate orchestration/failure cleanup, not database backup correctness.
No daemon, network, application database or actual CI credential is accessed.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

LAUNCHER = Path(__file__).with_name('test-mysql-multidb.sh')
STUB = r'''
import json,os,sys
from pathlib import Path
tool=Path(sys.argv[0]).name
args=sys.argv[1:]
p=Path(os.environ['FIXTURE_STATE'])
s=json.loads(p.read_text())
s['calls'].append([tool,*args])
mode=os.environ.get('FIXTURE_MODE','success')
code=0
if tool=='docker':
    if args[0]=='create':
        name=args[args.index('--name')+1]
        if mode=='collision': code=1
        else: s['containers'][name]=args[args.index('--label')+1].split('=',1)[1]
    elif args[0]=='inspect':
        print('unrelated-owner' if mode=='ownership-change' else s['containers'][args[-1]])
    elif args[0]=='exec':
        print('00000000-0000-4000-8000-00000000'+args[1][-4:])
    elif args[0]=='rm': s['containers'].pop(args[-1],None)
elif tool=='cargo' and mode=='test-failure': code=1
p.write_text(json.dumps(s))
sys.exit(code)
'''


class MysqlLauncherTests(unittest.TestCase):
    def launch(self, mode='success', ci=True):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); binary=root/'bin'; binary.mkdir()
            tool=binary/'stub'
            tool.write_text('#!'+sys.executable+'\n'+STUB); tool.chmod(0o700)
            for name in ['docker','mysql','mysqldump','cargo','sleep']:
                (binary/name).symlink_to(tool)
            state=root/'state.json'; state.write_text(json.dumps({'calls':[],'containers':{}}))
            env={**os.environ,'PATH':str(binary)+':'+os.environ['PATH'],
                 'FIXTURE_STATE':str(state),'FIXTURE_MODE':mode,
                 'GITHUB_ACTIONS':'true' if ci else 'false','RUNNER_ENVIRONMENT':'github-hosted',
                 'GITHUB_RUN_ID':'12345','GITHUB_RUN_ATTEMPT':'1'}
            result=subprocess.run(['bash',str(LAUNCHER)],env=env,capture_output=True,text=True,timeout=30)
            return result,json.loads(state.read_text())

    def test_success_uses_fresh_pair_per_case_and_removes_only_owned_fixtures(self):
        result,state=self.launch()
        self.assertEqual(result.returncode,0,result.stderr)
        creates=[c for c in state['calls'] if c[:2]==['docker','create']]
        self.assertEqual(len(creates),4)
        names=[c[c.index('--name')+1] for c in creates]
        self.assertEqual(len(set(names)),4)
        tests=[c for c in state['calls'] if c[0]=='cargo']
        self.assertEqual(len(tests),2)
        self.assertTrue(all('--exact' in c and '--ignored' in c for c in tests))
        removed=[c[-1] for c in state['calls'] if c[:2]==['docker','rm']]
        self.assertEqual(set(removed),set(names))
        self.assertEqual(state['containers'],{})

    def test_creation_collision_never_cleans_up_existing_container(self):
        result,state=self.launch('collision')
        self.assertNotEqual(result.returncode,0)
        self.assertFalse(any(c[:2]==['docker','rm'] or c[0]=='cargo' for c in state['calls']))

    def test_test_failure_stops_pipeline_and_cleans_only_its_pair(self):
        result,state=self.launch('test-failure')
        self.assertNotEqual(result.returncode,0)
        self.assertEqual(len([c for c in state['calls'] if c[0]=='cargo']),1)
        self.assertEqual(len([c for c in state['calls'] if c[:2]==['docker','rm']]),2)

    def test_changed_ownership_refuses_removal(self):
        result,state=self.launch('ownership-change')
        self.assertNotEqual(result.returncode,0)
        self.assertFalse(any(c[:2]==['docker','rm'] for c in state['calls']))

    def test_non_ci_host_refused_before_external_commands(self):
        result,state=self.launch(ci=False)
        self.assertEqual(result.returncode,1)
        self.assertEqual(state['calls'],[])


if __name__=='__main__': unittest.main()
