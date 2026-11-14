// SPDX-License-Identifier: Apache-2.0

package vfsbridge

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	seekable "github.com/a1ex3/zstd-seekable-format-go/pkg"
	"github.com/klauspost/compress/zstd"
	"github.com/rclone/rclone/backend/nievfs"
	"github.com/rclone/rclone/fs"
	"github.com/rclone/rclone/fs/cache"
	"github.com/rclone/rclone/fs/object"
	rcsync "github.com/rclone/rclone/fs/sync"
)

// ExportOptions tunes parallel VFS conversion, compression, and upload/download throughput.
type ExportOptions struct {
	LocalDataDir       string
	DstURI             string
	Mode               string // packs | decrypted-packs | seekable-zstd | loose
	DryRun             bool
	Transfers          int // concurrent file uploads/transfers (default 32)
	Checkers           int // concurrent remote directory checkers (default 64)
	MultiThreadStreams int // parallel streams per large file (default 16)
	ZstdLevel          int // zstd level for seekable-zstd (default 4)
	WatchSeconds       int // polling interval for WatchAndSyncPacks (default 15)
	MaxPacks           int // optional cap for smoke tests / progressive sync (0 = all)
	IncludeFilter      string
}

func (o *ExportOptions) withDefaults() ExportOptions {
	out := *o
	if out.Mode == "" {
		out.Mode = "packs"
	}
	if out.Transfers <= 0 {
		out.Transfers = 32
	}
	if out.Checkers <= 0 {
		out.Checkers = 64
	}
	if out.MultiThreadStreams <= 0 {
		out.MultiThreadStreams = 16
	}
	if out.ZstdLevel <= 0 {
		out.ZstdLevel = 4
	}
	if out.WatchSeconds <= 0 {
		out.WatchSeconds = 15
	}
	return out
}

func applyRcloneTuning(ctx context.Context, opts ExportOptions) {
	ci := fs.GetConfig(ctx)
	ci.Transfers = opts.Transfers
	ci.Checkers = opts.Checkers
	ci.MultiThreadStreams = opts.MultiThreadStreams
	ci.MultiThreadSet = true
	ci.MultiThreadCutoff = 32 * fs.Mebi
	ci.MultiThreadChunkSize = 16 * fs.Mebi
	ci.BufferSize = 64 * fs.Mebi
	ci.UseServerModTime = true
}

// xorDecryptReader streams Criware XOR decryption lazily in O(1) memory.
type xorDecryptReader struct {
	f      *os.File
	key    uint32
	offset uint64
}

func (r *xorDecryptReader) Read(p []byte) (int, error) {
	n, err := r.f.Read(p)
	if n > 0 {
		nievfs.DecryptBlock(p[:n], r.offset, r.key)
		r.offset += uint64(n)
	}
	return n, err
}

func encodeSeekableZstdPack(ctx context.Context, packPath, packName string, level int) ([]byte, error) {
	raw, err := os.ReadFile(packPath)
	if err != nil {
		return nil, err
	}
	nievfs.DecryptBlock(raw, 0, nievfs.KeyFromFilename(packName))

	enc, err := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.EncoderLevelFromZstd(level)))
	if err != nil {
		return nil, err
	}
	defer func() { _ = enc.Close() }()

	var out bytes.Buffer
	sw, err := seekable.NewWriter(&out, enc)
	if err != nil {
		return nil, err
	}
	const frameSize = 2 * 1024 * 1024
	off := 0
	var mu sync.Mutex
	err = sw.WriteMany(ctx, func() ([]byte, error) {
		mu.Lock()
		defer mu.Unlock()
		if off >= len(raw) {
			return nil, nil
		}
		end := off + frameSize
		if end > len(raw) {
			end = len(raw)
		}
		frame := raw[off:end]
		off = end
		return frame, nil
	}, seekable.WithConcurrency(max(2, min(runtime.NumCPU(), 8))))
	if err != nil {
		_ = sw.Close()
		return nil, err
	}
	if err := sw.Close(); err != nil {
		return nil, err
	}
	return out.Bytes(), nil
}

func listExistingSizes(ctx context.Context, dstFs fs.Fs, subDir string) map[string]int64 {
	sizes := make(map[string]int64)
	entries, err := dstFs.List(ctx, subDir)
	if err != nil {
		return sizes
	}
	for _, ent := range entries {
		if obj, ok := ent.(fs.Object); ok {
			sizes[obj.Remote()] = obj.Size()
		}
	}
	return sizes
}

// ConvertAndExportPacksWithOptions prepares and uploads IEVR data/packs in parallel with
// optional on-the-fly XOR decryption or 2 MiB seekable Zstd encoding.
func ConvertAndExportPacksWithOptions(ctx context.Context, rawOpts ExportOptions) (*PackExportReport, error) {
	t0 := time.Now()
	opts := rawOpts.withDefaults()
	applyRcloneTuning(ctx, opts)

	localDataDir := opts.LocalDataDir
	cfgPath := filepath.Join(localDataDir, "cpk_list.cfg.bin")
	if _, err := os.Stat(cfgPath); err != nil {
		sub := filepath.Join(localDataDir, "data", "cpk_list.cfg.bin")
		if _, err2 := os.Stat(sub); err2 != nil {
			return nil, fmt.Errorf("cpk_list.cfg.bin not found under %q", localDataDir)
		}
		localDataDir = filepath.Join(localDataDir, "data")
		cfgPath = sub
	}

	rawCfg, err := os.ReadFile(cfgPath)
	if err != nil {
		return nil, err
	}
	logicalEntries, err := nievfs.ParseCpkList(rawCfg)
	if err != nil {
		return nil, err
	}

	packsDir := filepath.Join(localDataDir, "packs")
	packEntries, err := os.ReadDir(packsDir)
	if err != nil {
		return nil, fmt.Errorf("failed to read packs directory %q: %w", packsDir, err)
	}
	filterLower := strings.ToLower(strings.TrimSpace(opts.IncludeFilter))
	var packNames []string
	var totalSrcBytes int64
	for _, ent := range packEntries {
		if ent.IsDir() || !strings.HasSuffix(strings.ToLower(ent.Name()), ".cpk") {
			continue
		}
		if filterLower != "" && !strings.Contains(strings.ToLower(ent.Name()), filterLower) {
			continue
		}
		packNames = append(packNames, ent.Name())
		if info, err := ent.Info(); err == nil {
			totalSrcBytes += info.Size()
		}
	}
	slices.Sort(packNames)
	if opts.MaxPacks > 0 && len(packNames) > opts.MaxPacks {
		packNames = packNames[:opts.MaxPacks]
	}

	resolvedDst, err := ResolveSpec(opts.DstURI)
	if err != nil {
		return nil, err
	}

	manifest := struct {
		Format         string   `json:"format"`
		Mode           string   `json:"mode"`
		PackCount      int      `json:"pack_count"`
		EntryCount     int      `json:"entry_count"`
		Transfers      int      `json:"transfers"`
		ZstdLevel      int      `json:"zstd_level,omitempty"`
		SeekableFrames bool     `json:"seekable_frames,omitempty"`
		RcloneMount    string   `json:"rclone_nievfs_spec"`
		Packs          []string `json:"packs"`
	}{
		Format:         "nie.rclone.cloud/v2",
		Mode:           opts.Mode,
		PackCount:      len(packNames),
		EntryCount:     len(logicalEntries),
		Transfers:      opts.Transfers,
		ZstdLevel:      opts.ZstdLevel,
		SeekableFrames: opts.Mode == "seekable-zstd",
		RcloneMount:    ":nievfs:" + resolvedDst,
		Packs:          packNames,
	}
	manifestBytes, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return nil, err
	}
	sum := sha256.Sum256(manifestBytes)
	manifestHash := hex.EncodeToString(sum[:])

	var copied int64
	var skipped int64
	var outputBytes int64

	if !opts.DryRun {
		dstFs, err := cache.Get(ctx, resolvedDst)
		if err != nil {
			return nil, fmt.Errorf("failed to open destination %q: %w", resolvedDst, err)
		}

		switch opts.Mode {
		case "packs":
			srcFs, err := cache.Get(ctx, localDataDir)
			if err != nil {
				return nil, err
			}
			if err := rcsync.CopyDir(ctx, dstFs, srcFs, false); err != nil {
				return nil, err
			}
			copied = int64(len(packNames) + 1)
			outputBytes = totalSrcBytes

		case "loose":
			srcFs, err := cache.Get(ctx, ":nievfs:"+localDataDir)
			if err != nil {
				return nil, err
			}
			if err := rcsync.CopyDir(ctx, dstFs, srcFs, false); err != nil {
				return nil, err
			}
			copied = int64(len(logicalEntries))

		case "decrypted-packs", "seekable-zstd":
			decCfg, err := nievfs.DecryptCpkList(rawCfg)
			if err != nil {
				return nil, fmt.Errorf("decrypt cpk_list.cfg.bin failed: %w", err)
			}
			cfgObjInfo := object.NewStaticObjectInfo("cpk_list.cfg.bin", time.Now(), int64(len(decCfg)), true, nil, dstFs)
			if _, err := dstFs.Put(ctx, bytes.NewReader(decCfg), cfgObjInfo); err != nil {
				return nil, fmt.Errorf("upload decrypted cpk_list.cfg.bin: %w", err)
			}
			copied++
			outputBytes += int64(len(decCfg))

			existing := listExistingSizes(ctx, dstFs, "packs")
			workers := min(max(1, opts.Transfers), max(1, len(packNames)))
			jobs := make(chan string, len(packNames))
			for _, p := range packNames {
				jobs <- p
			}
			close(jobs)

			var firstErr error
			var errOnce sync.Once
			var wg sync.WaitGroup
			ctxUpload, cancel := context.WithCancel(ctx)
			defer cancel()

			for range workers {
				wg.Add(1)
				go func() {
					defer wg.Done()
					for name := range jobs {
						if ctxUpload.Err() != nil {
							return
						}
						localPath := filepath.Join(packsDir, name)
						if opts.Mode == "seekable-zstd" {
							remoteRel := "packs/" + name + ".szst"
							encoded, err := encodeSeekableZstdPack(ctxUpload, localPath, name, opts.ZstdLevel)
							if err != nil {
								errOnce.Do(func() { firstErr = err; cancel() })
								return
							}
							if prevSize, ok := existing[remoteRel]; ok && prevSize == int64(len(encoded)) {
								atomic.AddInt64(&skipped, 1)
								atomic.AddInt64(&outputBytes, prevSize)
								continue
							}
							info := object.NewStaticObjectInfo(remoteRel, time.Now(), int64(len(encoded)), true, nil, dstFs)
							if _, err := dstFs.Put(ctxUpload, bytes.NewReader(encoded), info); err != nil {
								errOnce.Do(func() { firstErr = err; cancel() })
								return
							}
							atomic.AddInt64(&copied, 1)
							atomic.AddInt64(&outputBytes, int64(len(encoded)))
							continue
						}

						st, err := os.Stat(localPath)
						if err != nil {
							errOnce.Do(func() { firstErr = err; cancel() })
							return
						}
						remoteRel := "packs/" + name
						if prevSize, ok := existing[remoteRel]; ok && prevSize == st.Size() {
							atomic.AddInt64(&skipped, 1)
							atomic.AddInt64(&outputBytes, prevSize)
							continue
						}
						f, err := os.Open(localPath)
						if err != nil {
							errOnce.Do(func() { firstErr = err; cancel() })
							return
						}
						xr := &xorDecryptReader{f: f, key: nievfs.KeyFromFilename(name)}
						info := object.NewStaticObjectInfo(remoteRel, st.ModTime(), st.Size(), true, nil, dstFs)
						_, putErr := dstFs.Put(ctxUpload, xr, info)
						_ = f.Close()
						if putErr != nil {
							errOnce.Do(func() { firstErr = putErr; cancel() })
							return
						}
						atomic.AddInt64(&copied, 1)
						atomic.AddInt64(&outputBytes, st.Size())
					}
				}()
			}
			wg.Wait()
			if firstErr != nil {
				return nil, firstErr
			}

		default:
			return nil, fmt.Errorf("unsupported export mode %q (expected packs, decrypted-packs, seekable-zstd, or loose)", opts.Mode)
		}

		info := object.NewStaticObjectInfo("nievfs-manifest.json", time.Now(), int64(len(manifestBytes)), true, nil, dstFs)
		if _, err := dstFs.Put(ctx, bytes.NewReader(manifestBytes), info); err != nil {
			return nil, err
		}
	}

	sample := packNames
	if len(sample) > 5 {
		sample = sample[:5]
	}
	return &PackExportReport{
		Mode:             opts.Mode,
		Source:           localDataDir,
		Destination:      opts.DstURI,
		ResolvedDst:      resolvedDst,
		PackCount:        len(packNames),
		LogicalCount:     len(logicalEntries),
		CopiedObjects:    int(copied),
		SkippedObjects:   int(skipped),
		TotalSourceBytes: totalSrcBytes,
		TotalOutputBytes: outputBytes,
		Transfers:        opts.Transfers,
		DurationMs:       time.Since(t0).Milliseconds(),
		ManifestPath:     resolvedDst + "/nievfs-manifest.json",
		ManifestHash:     manifestHash,
		SamplePacks:      sample,
	}, nil
}

// WatchAndSyncPacks continuously polls localDataDir until ctx is cancelled, syncing
// any new or changed packs immediately while skipping identical remote objects.
func WatchAndSyncPacks(ctx context.Context, rawOpts ExportOptions, onCycle func(*PackExportReport)) error {
	opts := rawOpts.withDefaults()
	interval := time.Duration(opts.WatchSeconds) * time.Second
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	var lastHash string
	for {
		report, err := ConvertAndExportPacksWithOptions(ctx, opts)
		if err != nil {
			return err
		}
		if report.ManifestHash != lastHash || report.CopiedObjects > 0 {
			lastHash = report.ManifestHash
			if onCycle != nil {
				onCycle(report)
			}
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
		}
	}
}
