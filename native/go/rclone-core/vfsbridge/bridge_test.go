// SPDX-License-Identifier: Apache-2.0

package vfsbridge

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	_ "github.com/rclone/rclone/backend/all"
	"github.com/rclone/rclone/fs/config/configfile"
)

func TestResolveSpecAndSecurity(t *testing.T) {
	configfile.Install()

	for _, unsafe := range []string{
		"http://user:pass@host/data",
		"http://user@host/data",
		"https://host/data?token=secret",
		"rclone://vps/data#frag",
		"r2://user:secret@bucket/path",
	} {
		if _, err := ResolveSpec(unsafe); err == nil {
			t.Fatalf("expected unsafe URI %q to be rejected", unsafe)
		}
	}

	t.Setenv("R2_ACCESS_KEY_ID", "test-ak")
	t.Setenv("R2_SECRET_ACCESS_KEY", "test-sk")
	t.Setenv("R2_ACCOUNT_ID", "acct123")
	r2Spec, err := ResolveSpec("r2://rosegriffon/ievr")
	if err != nil {
		t.Fatalf("ResolveSpec(r2) failed: %v", err)
	}
	if !strings.HasPrefix(r2Spec, ":s3,provider=Cloudflare,") || !strings.HasSuffix(r2Spec, ":rosegriffon/ievr") {
		t.Fatalf("unexpected r2 spec: %s", r2Spec)
	}

	rcSpec, err := ResolveSpec("rclone://gdrive-ai/ievr/data")
	if err != nil || rcSpec != "gdrive-ai:ievr/data" {
		t.Fatalf("unexpected rclone spec: %q err=%v", rcSpec, err)
	}

	gdSpec, err := ResolveSpec("gdrive://gdrive-ai/ievr")
	if err != nil || gdSpec != "gdrive-ai:ievr" {
		t.Fatalf("unexpected gdrive spec: %q err=%v", gdSpec, err)
	}

	nvSpec, err := ResolveSpec("nievfs://gdrive-ai/ievr/data")
	if err != nil || !strings.HasPrefix(nvSpec, ":nievfs:") {
		t.Fatalf("unexpected nievfs spec: %q err=%v", nvSpec, err)
	}
}

func TestReadRangeListDirAndRealPacksDryRun(t *testing.T) {
	configfile.Install()
	ctx := context.Background()

	tmp := t.TempDir()
	sample := filepath.Join(tmp, "packs", "test.cpk")
	if err := os.MkdirAll(filepath.Dir(sample), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sample, []byte("0123456789"), 0o644); err != nil {
		t.Fatal(err)
	}
	got, err := ReadRemoteRange(ctx, tmp, "packs/test.cpk", 3, 4)
	if err != nil || string(got) != "3456" {
		t.Fatalf("ReadRemoteRange got %q err=%v", string(got), err)
	}
	items, err := ListRemoteDir(ctx, tmp, "")
	if err != nil || len(items) != 1 || items[0] != "packs/" {
		t.Fatalf("ListRemoteDir got %v err=%v", items, err)
	}

	steamData := `C:\Program Files (x86)\Steam\steamapps\common\INAZUMA ELEVEN Victory Road\data`
	if _, err := os.Stat(filepath.Join(steamData, "cpk_list.cfg.bin")); err == nil {
		t.Setenv("R2_ACCESS_KEY_ID", "dry-ak")
		t.Setenv("R2_SECRET_ACCESS_KEY", "dry-sk")
		t.Setenv("R2_ACCOUNT_ID", "dry-acct")
		report, err := ConvertAndExportPacks(ctx, steamData, "r2://rosegriffon-ievr/data", "packs", true)
		if err != nil {
			t.Fatalf("ConvertAndExportPacks dry-run failed: %v", err)
		}
		if report.PackCount < 900 || report.LogicalCount < 200000 {
			t.Fatalf("unexpected real pack/logical counts: %+v", report)
		}
		t.Logf("verified real packs dry-run: packs=%d logical=%d hash=%s", report.PackCount, report.LogicalCount, report.ManifestHash)

		outDec := t.TempDir()
		repDec, err := ConvertAndExportPacksWithOptions(ctx, ExportOptions{
			LocalDataDir: steamData,
			DstURI:       outDec,
			Mode:         "decrypted-packs",
			Transfers:    8,
			MaxPacks:     2,
		})
		if err != nil {
			t.Fatalf("decrypted-packs real sample failed: %v", err)
		}
		if repDec.CopiedObjects != 3 || len(repDec.SamplePacks) != 2 {
			t.Fatalf("unexpected decrypted-packs report: %+v", repDec)
		}
		firstPack, err := os.ReadFile(filepath.Join(outDec, "packs", repDec.SamplePacks[0]))
		if err != nil || len(firstPack) < 0x14 || string(firstPack[:4]) != "CPK " || string(firstPack[0x10:0x14]) != "@UTF" {
			t.Fatalf("exported decrypted pack does not start with CPK + @UTF: err=%v len=%d", err, len(firstPack))
		}

		outSzst := t.TempDir()
		repSzst, err := ConvertAndExportPacksWithOptions(ctx, ExportOptions{
			LocalDataDir: steamData,
			DstURI:       outSzst,
			Mode:         "seekable-zstd",
			Transfers:    8,
			ZstdLevel:    4,
			MaxPacks:     2,
		})
		if err != nil {
			t.Fatalf("seekable-zstd real sample failed: %v", err)
		}
		if repSzst.CopiedObjects != 3 || repSzst.TotalOutputBytes >= repSzst.TotalSourceBytes+1024*1024 {
			t.Fatalf("unexpected seekable-zstd report: %+v", repSzst)
		}
		if _, err := os.Stat(filepath.Join(outSzst, "packs", repSzst.SamplePacks[0]+".szst")); err != nil {
			t.Fatalf("missing .szst file: %v", err)
		}
	}
}
