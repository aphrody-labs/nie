// SPDX-License-Identifier: Apache-2.0

package main

import (
	"bufio"
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/aphrody-labs/rosegriffon/native/go/rclone-core/vfsbridge"
	_ "github.com/rclone/rclone/backend/all"
	"github.com/rclone/rclone/cmd"
	_ "github.com/rclone/rclone/cmd/all"
	"github.com/rclone/rclone/fs/config"
	"github.com/rclone/rclone/fs/config/configfile"
	_ "github.com/rclone/rclone/lib/plugin"
)

type rpcReq struct {
	Op     string `json:"op"`
	URI    string `json:"uri"`
	Path   string `json:"path"`
	Offset int64  `json:"offset"`
	Length int64  `json:"length"`
}

type rpcResp struct {
	Ok    bool     `json:"ok"`
	Len   int      `json:"len,omitempty"`
	Items []string `json:"items,omitempty"`
	Error string   `json:"error,omitempty"`
}

func runRPC() {
	configfile.Install()
	reader := bufio.NewReaderSize(os.Stdin, 64*1024)
	ctx := context.Background()
	writeResp := func(resp rpcResp, payload []byte) {
		line, _ := json.Marshal(resp)
		line = append(line, '\n')
		_, _ = os.Stdout.Write(line)
		if len(payload) > 0 {
			_, _ = os.Stdout.Write(payload)
		}
	}
	for {
		line, err := reader.ReadString('\n')
		if len(line) > 0 {
			trimmed := strings.TrimSpace(line)
			if trimmed != "" {
				var req rpcReq
				if jerr := json.Unmarshal([]byte(trimmed), &req); jerr != nil {
					writeResp(rpcResp{Ok: false, Error: jerr.Error()}, nil)
				} else {
					switch req.Op {
					case "read":
						data, rerr := vfsbridge.ReadRemoteRange(ctx, req.URI, req.Path, req.Offset, req.Length)
						if rerr != nil {
							writeResp(rpcResp{Ok: false, Error: rerr.Error()}, nil)
						} else {
							writeResp(rpcResp{Ok: true, Len: len(data)}, data)
						}
					case "list":
						items, lerr := vfsbridge.ListRemoteDir(ctx, req.URI, req.Path)
						if lerr != nil {
							writeResp(rpcResp{Ok: false, Error: lerr.Error()}, nil)
						} else {
							writeResp(rpcResp{Ok: true, Items: items}, nil)
						}
					default:
						writeResp(rpcResp{Ok: false, Error: "unknown op: " + req.Op}, nil)
					}
				}
			}
		}
		if err != nil {
			if err != io.EOF {
				fmt.Fprintln(os.Stderr, "rpc read error:", err)
			}
			return
		}
	}
}

func main() {
	if len(os.Args) >= 2 {
		switch os.Args[1] {
		case "nie-remotes":
			configfile.Install()
			remotes := config.GetRemotes()
			for i := range remotes {
				remotes[i].Type = strings.TrimSpace(remotes[i].Type)
			}
			enc := json.NewEncoder(os.Stdout)
			_ = enc.Encode(remotes)
			return
		case "nie-resolve":
			if len(os.Args) < 3 {
				fmt.Fprintln(os.Stderr, "usage: nie-rclone nie-resolve <uri>")
				os.Exit(2)
			}
			spec, err := vfsbridge.ResolveSpec(os.Args[2])
			if err != nil {
				fmt.Fprintln(os.Stderr, "error:", err)
				os.Exit(1)
			}
			fmt.Println(spec)
			return
		case "nie-rpc":
			runRPC()
			return
		case "nie-export", "nie-watch":
			watchMode := os.Args[1] == "nie-watch"
			configfile.Install()
			fsFlag := flag.NewFlagSet(os.Args[1], flag.ExitOnError)
			src := fsFlag.String("source", "", "Local IEVR game or data directory")
			dst := fsFlag.String("dest", "", "Destination URI (r2://, s3://, gdrive://, rclone://, or path)")
			mode := fsFlag.String("mode", "decrypted-packs", "Export mode: packs | decrypted-packs | seekable-zstd | loose")
			transfers := fsFlag.Int("transfers", 32, "Concurrent file uploads/transfers")
			checkers := fsFlag.Int("checkers", 64, "Concurrent remote checkers")
			streams := fsFlag.Int("multi-thread-streams", 16, "Concurrent multi-thread streams per large pack")
			zstdLevel := fsFlag.Int("zstd-level", 4, "Zstd compression level for seekable-zstd mode")
			maxPacks := fsFlag.Int("max-packs", 0, "Optional maximum number of packs per run (0 = all)")
			filter := fsFlag.String("filter", "", "Optional substring filter for pack filenames")
			watchSecs := fsFlag.Int("interval", 15, "Watch poll interval in seconds (nie-watch)")
			dryRun := fsFlag.Bool("dry-run", false, "Validate and generate manifest without uploading")
			_ = fsFlag.Parse(os.Args[2:])
			if *src == "" || *dst == "" {
				fmt.Fprintln(os.Stderr, "usage: nie-rclone nie-export|nie-watch --source <dir> --dest <uri> [--mode packs|decrypted-packs|seekable-zstd|loose] [--transfers 32] [--checkers 64] [--dry-run]")
				os.Exit(2)
			}
			opts := vfsbridge.ExportOptions{
				LocalDataDir:       *src,
				DstURI:             *dst,
				Mode:               *mode,
				Transfers:          *transfers,
				Checkers:           *checkers,
				MultiThreadStreams: *streams,
				ZstdLevel:          *zstdLevel,
				MaxPacks:           *maxPacks,
				IncludeFilter:      *filter,
				WatchSeconds:       *watchSecs,
				DryRun:             *dryRun,
			}
			enc := json.NewEncoder(os.Stdout)
			enc.SetIndent("", "  ")
			if watchMode {
				if err := vfsbridge.WatchAndSyncPacks(context.Background(), opts, func(r *vfsbridge.PackExportReport) {
					_ = enc.Encode(r)
				}); err != nil {
					fmt.Fprintln(os.Stderr, "error:", err)
					os.Exit(1)
				}
				return
			}
			report, err := vfsbridge.ConvertAndExportPacksWithOptions(context.Background(), opts)
			if err != nil {
				fmt.Fprintln(os.Stderr, "error:", err)
				os.Exit(1)
			}
			_ = enc.Encode(report)
			return
		}
	}
	cmd.Main()
}
