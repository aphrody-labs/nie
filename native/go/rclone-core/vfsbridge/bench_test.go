// SPDX-License-Identifier: Apache-2.0

package vfsbridge

import (
	"bytes"
	"context"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"testing"
	"time"

	seekable "github.com/a1ex3/zstd-seekable-format-go/pkg"
	"github.com/andybalholm/brotli"
	"github.com/klauspost/compress/s2"
	"github.com/klauspost/compress/zstd"
	"github.com/pierrec/lz4/v4"
	"github.com/rclone/rclone/backend/nievfs"
)

func loadRealPackSample(t *testing.T, maxBytes int64) (rawBuf, decBuf []byte, count int) {
	t.Helper()
	packsDir := `C:\Program Files (x86)\Steam\steamapps\common\INAZUMA ELEVEN Victory Road\data\packs`
	entries, err := os.ReadDir(packsDir)
	if err != nil {
		t.Skipf("IEVR packs directory unavailable: %v", err)
	}
	var names []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(strings.ToLower(e.Name()), ".cpk") {
			info, err := e.Info()
			if err == nil && info.Size() <= 8*1024*1024 {
				names = append(names, e.Name())
			}
		}
	}
	slices.Sort(names)

	var rawOut, decOut bytes.Buffer
	for _, name := range names {
		if int64(rawOut.Len()) >= maxBytes {
			break
		}
		b, err := os.ReadFile(filepath.Join(packsDir, name))
		if err != nil {
			continue
		}
		rawOut.Write(b)
		dec := make([]byte, len(b))
		copy(dec, b)
		key := nievfs.KeyFromFilename(name)
		nievfs.DecryptBlock(dec, 0, key)
		decOut.Write(dec)
		count++
	}
	if count == 0 {
		t.Skip("no suitable sample CPKs found")
	}
	return rawOut.Bytes(), decOut.Bytes(), count
}

func TestCompressionAlgorithmsOnRealPacks(t *testing.T) {
	rawSample, decSample, packCount := loadRealPackSample(t, 32*1024*1024)
	t.Logf("Loaded %d real CPK packs (%d bytes = %.2f MiB)", packCount, len(rawSample), float64(len(rawSample))/(1024*1024))

	runCase := func(label string, src []byte, compressFn func([]byte) ([]byte, error), decompressFn func([]byte) ([]byte, error)) {
		t.Helper()
		t0 := time.Now()
		comp, err := compressFn(src)
		encDur := time.Since(t0)
		if err != nil {
			t.Fatalf("%s compress error: %v", label, err)
		}
		t1 := time.Now()
		back, err := decompressFn(comp)
		decDur := time.Since(t1)
		if err != nil {
			t.Fatalf("%s decompress error: %v", label, err)
		}
		if len(back) != len(src) {
			t.Fatalf("%s roundtrip length mismatch: %d != %d", label, len(back), len(src))
		}
		mib := float64(len(src)) / (1024 * 1024)
		ratio := float64(len(comp)) / float64(len(src)) * 100.0
		t.Logf("%-28s | out=%8.2f MiB (%5.2f%%) | enc=%7.1f MiB/s (%s) | dec=%7.1f MiB/s (%s)",
			label,
			float64(len(comp))/(1024*1024),
			ratio,
			mib/encDur.Seconds(), encDur.Round(time.Millisecond),
			mib/decDur.Seconds(), decDur.Round(time.Millisecond),
		)
	}

	zstdCompress := func(lvl int) func([]byte) ([]byte, error) {
		return func(src []byte) ([]byte, error) {
			enc, err := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.EncoderLevelFromZstd(lvl)), zstd.WithEncoderConcurrency(runtime.NumCPU()))
			if err != nil {
				return nil, err
			}
			defer func() { _ = enc.Close() }()
			return enc.EncodeAll(src, make([]byte, 0, len(src))), nil
		}
	}
	zstdDecompress := func(comp []byte) ([]byte, error) {
		dec, err := zstd.NewReader(nil, zstd.WithDecoderConcurrency(runtime.NumCPU()))
		if err != nil {
			return nil, err
		}
		defer dec.Close()
		return dec.DecodeAll(comp, nil)
	}

	// 1. Control: encrypted raw CPKs with Zstd level 4 (shows why decrypting before compress matters!)
	runCase("encrypted-cpk + zstd-4", rawSample, zstdCompress(4), zstdDecompress)

	// 2. Decrypted CPKs across Zstd levels
	runCase("decrypted-cpk + zstd-1", decSample, zstdCompress(1), zstdDecompress)
	runCase("decrypted-cpk + zstd-4", decSample, zstdCompress(4), zstdDecompress)
	runCase("decrypted-cpk + zstd-9", decSample, zstdCompress(9), zstdDecompress)

	// 3. Seekable Zstd (2 MiB frames) for O(1) random range streaming
	runCase("decrypted + seekable-zstd-4", decSample, func(src []byte) ([]byte, error) {
		var out bytes.Buffer
		enc, err := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.EncoderLevelFromZstd(4)))
		if err != nil {
			return nil, err
		}
		defer func() { _ = enc.Close() }()
		sw, err := seekable.NewWriter(&out, enc)
		if err != nil {
			return nil, err
		}
		const frameSize = 2 * 1024 * 1024
		off := 0
		err = sw.WriteMany(context.Background(), func() ([]byte, error) {
			if off >= len(src) {
				return nil, nil
			}
			end := off + frameSize
			if end > len(src) {
				end = len(src)
			}
			chunk := src[off:end]
			off = end
			return chunk, nil
		}, seekable.WithConcurrency(runtime.NumCPU()))
		if err != nil {
			_ = sw.Close()
			return nil, err
		}
		if err := sw.Close(); err != nil {
			return nil, err
		}
		return out.Bytes(), nil
	}, func(comp []byte) ([]byte, error) {
		dec, err := zstd.NewReader(nil)
		if err != nil {
			return nil, err
		}
		defer dec.Close()
		sr, err := seekable.NewReader(bytes.NewReader(comp), dec)
		if err != nil {
			return nil, err
		}
		defer func() { _ = sr.Close() }()
		return io.ReadAll(sr)
	})

	// 4. LZ4, S2, Brotli-4 on decrypted CPKs
	runCase("decrypted-cpk + lz4", decSample, func(src []byte) ([]byte, error) {
		var out bytes.Buffer
		zw := lz4.NewWriter(&out)
		_ = zw.Apply(lz4.ConcurrencyOption(runtime.NumCPU()))
		if _, err := zw.Write(src); err != nil {
			return nil, err
		}
		if err := zw.Close(); err != nil {
			return nil, err
		}
		return out.Bytes(), nil
	}, func(comp []byte) ([]byte, error) {
		return io.ReadAll(lz4.NewReader(bytes.NewReader(comp)))
	})

	runCase("decrypted-cpk + s2", decSample, func(src []byte) ([]byte, error) {
		return s2.Encode(nil, src), nil
	}, func(comp []byte) ([]byte, error) {
		return s2.Decode(nil, comp)
	})

	runCase("decrypted-cpk + brotli-4", decSample, func(src []byte) ([]byte, error) {
		var out bytes.Buffer
		bw := brotli.NewWriterLevel(&out, 4)
		if _, err := bw.Write(src); err != nil {
			return nil, err
		}
		if err := bw.Close(); err != nil {
			return nil, err
		}
		return out.Bytes(), nil
	}, func(comp []byte) ([]byte, error) {
		return io.ReadAll(brotli.NewReader(bytes.NewReader(comp)))
	})

	// 5. Decrypted logical VFS files (CRILAYLA-decompressed cfg.bin/g4* assets via nievfs)
	dataDir := `C:\Program Files (x86)\Steam\steamapps\common\INAZUMA ELEVEN Victory Road\data`
	if rawCfg, err := os.ReadFile(filepath.Join(dataDir, "cpk_list.cfg.bin")); err == nil {
		if decCfg, err := nievfs.DecryptCpkList(rawCfg); err == nil {
			runCase("cpk_list.cfg.bin (dec)+zstd4", decCfg, zstdCompress(4), zstdDecompress)
			runCase("cpk_list.cfg.bin (dec)+zstd9", decCfg, zstdCompress(9), zstdDecompress)
		}
	}
	if _, err := os.Stat(filepath.Join(dataDir, "cpk_list.cfg.bin")); err == nil {
		ctx := context.Background()
		spec := ":nievfs:" + dataDir
		if topDirs, err := ListRemoteDir(ctx, spec, "common"); err == nil && len(topDirs) > 0 {
			var logicalBuf bytes.Buffer
			for _, sub := range topDirs {
				if logicalBuf.Len() >= 8*1024*1024 {
					break
				}
				if !strings.HasSuffix(sub, "/") {
					if b, err := ReadRemoteRange(ctx, spec, "common/"+sub, 0, -1); err == nil {
						logicalBuf.Write(b)
					}
					continue
				}
				if children, err := ListRemoteDir(ctx, spec, "common/"+strings.TrimSuffix(sub, "/")); err == nil {
					for _, c := range children {
						if logicalBuf.Len() >= 8*1024*1024 {
							break
						}
						if !strings.HasSuffix(c, "/") {
							if b, err := ReadRemoteRange(ctx, spec, "common/"+sub+c, 0, -1); err == nil {
								logicalBuf.Write(b)
							}
						}
					}
				}
			}
			if logicalBuf.Len() > 0 {
				ls := logicalBuf.Bytes()
				t.Logf("Loaded %d bytes (%.2f MiB) of CRILAYLA-decompressed logical VFS files from common/", len(ls), float64(len(ls))/(1024*1024))
				runCase("logical-vfs + zstd-4", ls, zstdCompress(4), zstdDecompress)
				runCase("logical-vfs + zstd-9", ls, zstdCompress(9), zstdDecompress)
			}
		}
	}
}
