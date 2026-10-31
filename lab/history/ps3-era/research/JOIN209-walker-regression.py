import importlib.util, io, json, subprocess, sys, types, unittest
from pathlib import Path
repo=Path('F:/d1/d1xenia')
sys.path.insert(0,str(repo/'scripts'))
import d1_walker
spec=importlib.util.spec_from_file_location('precision_tests',repo/'scripts/test_d1_walker.py')
tests=importlib.util.module_from_spec(spec);spec.loader.exec_module(tests)
prior=types.ModuleType('walker_prior')
exec(compile(subprocess.check_output(['git','-C',str(repo),'show','HEAD:scripts/d1_walker.py'],text=True),'prior_walker','exec'),prior.__dict__)
results=[]
for label,walker in [('prior',prior.Walker),('candidate',d1_walker.Walker)]:
    tests.Walker=walker
    stream=io.StringIO()
    result=unittest.TextTestRunner(stream=stream).run(unittest.defaultTestLoader.loadTestsFromTestCase(tests.WalkerPrecision))
    results.append({'scope':label,'tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'output':stream.getvalue()})
if results[0]['failures']!=1 or results[0]['errors'] or results[1]['failures'] or results[1]['errors']:
    raise RuntimeError('Unexpected counterfactual result')
Path(__file__).with_suffix('.json').write_text(json.dumps(results,indent=2)+'\n')
print('PASS: precision regression fails with prior walker and passes with candidate.')
