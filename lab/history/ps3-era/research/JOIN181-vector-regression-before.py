import importlib.util, sys, subprocess, unittest
from pathlib import Path
root=Path(__file__).parent
original=subprocess.run(['git','show','HEAD:server/investment.py'],cwd='F:/d1/d1xenia',capture_output=True,check=True).stdout
path=root/'JOIN181-investment-before.py'
assert not path.exists()
path.write_bytes(original)
sys.path.insert(0,'F:/d1/d1xenia/server')
spec=importlib.util.spec_from_file_location('investment',path)
module=importlib.util.module_from_spec(spec)
sys.modules['investment']=module
spec.loader.exec_module(module)
suite=unittest.defaultTestLoader.loadTestsFromName('test_investment_positions')
result=unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(0 if not result.wasSuccessful() else 1)
