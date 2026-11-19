package nie_test

import (
	"os"
	"strings"
	"testing"

	nie "nie.local/smoke"
)

func TestSmoke(t *testing.T) {
	if nie.AbiVersion() != nie.AbiVersionExpected {
		t.Fatalf("abi %d", nie.AbiVersion())
	}
	data, err := os.ReadFile(os.Getenv("NIE_FIXTURE"))
	if err != nil {
		t.Fatal(err)
	}
	sql, ok := nie.CfgbinToSql(data, 1)
	if !ok || !strings.Contains(sql, "CREATE TABLE") || !strings.Contains(sql, "ROOT") {
		t.Fatalf("bad sql %q", sql)
	}
	if _, ok := nie.CfgbinToSql([]byte("nope"), 1); ok {
		t.Fatal("garbage accepted")
	}
	t.Logf("go ok %d %d", nie.AbiVersion(), len(sql))
}
