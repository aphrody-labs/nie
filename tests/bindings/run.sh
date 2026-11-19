#!/bin/sh
# Smoke test of every generated binding against the cdylib of the contract.
#   NIE_LIB       path of libnie.so / nie.dll / libnie.dylib (required)
#   NIE_BINDINGS  directory written by `nie bindgen` (default: <repo>/bindings)
#   NIE_FIXTURE   written by this script from fixture.hex into a temp directory
#   ZIG           zig binary (default: zig)
# Each language is run when its toolchain exists; the summary names what ran and what was skipped.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
: "${NIE_LIB:?set NIE_LIB to the shared library}"
NIE_BINDINGS=${NIE_BINDINGS:-$root/bindings}
ZIG=${ZIG:-zig}
libdir=$(dirname "$NIE_LIB")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
NIE_FIXTURE=$tmp/fixture.bin
export NIE_LIB NIE_BINDINGS NIE_FIXTURE
export LD_LIBRARY_PATH="$libdir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
python3 -I -c "import sys,binascii;open(sys.argv[2],'wb').write(binascii.unhexlify(open(sys.argv[1]).read().strip()))" "$here/fixture.hex" "$NIE_FIXTURE" || exit 1
ran=""; skipped=""; failed=""
step() { # name toolchain command...
  name=$1; tool=$2; shift 2
  if ! command -v "$tool" >/dev/null 2>&1; then skipped="$skipped $name($tool missing)"; return; fi
  if out=$("$@" 2>&1); then echo "$out" | tail -1; ran="$ran $name"; else echo "FAIL $name"; echo "$out" | tail -20; failed="$failed $name"; fi
}
step c cc sh -c "cc -I'$NIE_BINDINGS/c' '$here/smoke.c' -L'$libdir' -lnie -o '$tmp/c' && '$tmp/c' '$NIE_FIXTURE'"
step cpp c++ sh -c "c++ -std=c++20 -I'$NIE_BINDINGS/cpp' '$here/smoke.cpp' -L'$libdir' -lnie -o '$tmp/cpp' && '$tmp/cpp' '$NIE_FIXTURE'"
step rust rustc sh -c "rustc --edition 2024 -L '$libdir' -l nie '$here/smoke_rs.rs' -o '$tmp/rs' && '$tmp/rs' '$NIE_FIXTURE'"
step python python3 python3 -I "$here/smoke.py" "$NIE_FIXTURE"
step bun bun bun "$here/smoke.ts" "$NIE_FIXTURE"
step go go sh -c "mkdir -p '$tmp/gomod/go' '$tmp/gomod/c' && cp '$NIE_BINDINGS/go/nie_contract.go' '$here/go/smoke_test.go' '$tmp/gomod/go/' && cp '$NIE_BINDINGS/c/nie_contract.h' '$tmp/gomod/c/' && printf 'module nie.local/smoke\ngo 1.24\n' > '$tmp/gomod/go/go.mod' && cd '$tmp/gomod/go' && CGO_LDFLAGS=\"-L$libdir\" go test -count=1 -v ./... 2>&1 | grep -E 'FAIL|^ok '"
step csharp dotnet sh -c "mkdir -p '$tmp/cs' && cp '$here/csharp/Program.cs' '$here/csharp/smoke.csproj' '$tmp/cs/' && sed -i 's#../../../bindings/csharp/NieContract.cs#$NIE_BINDINGS/csharp/NieContract.cs#' '$tmp/cs/smoke.csproj' && cd '$tmp/cs' && DOTNET_NOLOGO=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 dotnet run -c Release -- '$NIE_FIXTURE'"
step zig "$ZIG" sh -c "mkdir -p '$tmp/zig' && cp '$here/zig/smoke.zig' '$tmp/zig/' && cd '$tmp/zig' && '$ZIG' build-exe --dep nie_contract -Mroot=smoke.zig -Mnie_contract='$NIE_BINDINGS/zig/nie_contract.zig' -lc -lnie -L'$libdir' -femit-bin='$tmp/zig/smoke' && ./smoke"
echo "ran:$ran"
[ -n "$skipped" ] && echo "skipped:$skipped"
[ -z "$failed" ] || { echo "failed:$failed"; exit 1; }
