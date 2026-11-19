"""Python (ctypes) binding smoke test: abi_version and cfgbin_to_sql on a synthetic fixture."""
import os
import sys

sys.path.insert(0, os.path.join(os.environ["NIE_BINDINGS"], "python"))
import nie_contract  # noqa: E402

lib = nie_contract.Library()
assert lib.abi_version() == nie_contract.ABI_VERSION, lib.abi_version()
data = open(sys.argv[1], "rb").read()
sql = lib.cfgbin_to_sql(data, 1)
assert sql is not None and "CREATE TABLE" in sql and "ROOT" in sql, sql
assert lib.cfgbin_to_sql(b"nope", 1) is None
print("python ok", lib.abi_version(), len(sql))
