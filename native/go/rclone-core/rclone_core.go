// SPDX-License-Identifier: Apache-2.0

package main

/*
#include <stdint.h>
#include <stdlib.h>

struct RcloneRPCResult {
	char* Output;
	int   Status;
};
*/
import "C"

import (
	"context"
	"encoding/json"
	"strings"
	"sync"
	"unsafe"

	"github.com/aphrody-labs/rosegriffon/native/go/rclone-core/vfsbridge"
	_ "github.com/rclone/rclone/backend/all"
	"github.com/rclone/rclone/fs/config"
	_ "github.com/rclone/rclone/fs/operations"
	_ "github.com/rclone/rclone/fs/sync"
	_ "github.com/rclone/rclone/lib/plugin"
	"github.com/rclone/rclone/librclone/librclone"
)

var initOnce sync.Once

func ensureInitialized() {
	initOnce.Do(func() {
		librclone.Initialize()
	})
}

func allocCBytes(data []byte) *C.char {
	if len(data) == 0 {
		return nil
	}
	ptr := C.malloc(C.size_t(len(data)))
	copy(unsafe.Slice((*byte)(ptr), len(data)), data)
	return (*C.char)(ptr)
}

//export RcloneInitialize
func RcloneInitialize() {
	ensureInitialized()
}

//export RcloneFinalize
func RcloneFinalize() {
	librclone.Finalize()
}

//export RcloneRPC
func RcloneRPC(method *C.char, input *C.char) (result C.struct_RcloneRPCResult) {
	ensureInitialized()
	output, status := librclone.RPC(C.GoString(method), C.GoString(input))
	result.Output = C.CString(output)
	result.Status = C.int(status)
	return result
}

//export RcloneFreeString
func RcloneFreeString(str *C.char) {
	if str != nil {
		C.free(unsafe.Pointer(str))
	}
}

//export RgRcloneVersion
func RgRcloneVersion() C.int32_t {
	return 100
}

//export RgRcloneFreeBuffer
func RgRcloneFreeBuffer(ptr unsafe.Pointer) {
	if ptr != nil {
		C.free(ptr)
	}
}

//export RgRcloneListRemotes
func RgRcloneListRemotes(outJSON **C.char, outLen *C.int64_t) C.int {
	ensureInitialized()
	remotes := config.GetRemotes()
	type remoteInfo struct {
		Name string `json:"name"`
		Type string `json:"type"`
	}
	list := make([]remoteInfo, 0, len(remotes))
	for _, r := range remotes {
		list = append(list, remoteInfo{Name: r.Name, Type: r.Type})
	}
	b, _ := json.Marshal(list)
	*outJSON = C.CString(string(b))
	if outLen != nil {
		*outLen = C.int64_t(len(b))
	}
	return 0
}

//export RgRcloneResolveSpec
func RgRcloneResolveSpec(uri *C.char, outSpec **C.char, outErr **C.char) C.int {
	ensureInitialized()
	resolved, err := vfsbridge.ResolveSpec(C.GoString(uri))
	if err != nil {
		*outErr = C.CString(err.Error())
		return 1
	}
	*outSpec = C.CString(resolved)
	return 0
}

//export RgRcloneReadRange
func RgRcloneReadRange(uri *C.char, relPath *C.char, offset C.int64_t, length C.int64_t, outData **C.char, outLen *C.int64_t, outErr **C.char) C.int {
	ensureInitialized()
	data, err := vfsbridge.ReadRemoteRange(context.Background(), C.GoString(uri), C.GoString(relPath), int64(offset), int64(length))
	if err != nil {
		*outErr = C.CString(err.Error())
		return 1
	}
	*outData = allocCBytes(data)
	*outLen = C.int64_t(len(data))
	return 0
}

//export RgRcloneListDir
func RgRcloneListDir(uri *C.char, relPath *C.char, outJSON **C.char, outLen *C.int64_t, outErr **C.char) C.int {
	ensureInitialized()
	items, err := vfsbridge.ListRemoteDir(context.Background(), C.GoString(uri), C.GoString(relPath))
	if err != nil {
		*outErr = C.CString(err.Error())
		return 1
	}
	b, _ := json.Marshal(items)
	*outJSON = C.CString(string(b))
	if outLen != nil {
		*outLen = C.int64_t(len(b))
	}
	return 0
}

//export RgRcloneExportPacks
func RgRcloneExportPacks(localDataDir *C.char, dstURI *C.char, mode *C.char, dryRun C.int, outJSON **C.char, outErr **C.char) C.int {
	ensureInitialized()
	report, err := vfsbridge.ConvertAndExportPacks(
		context.Background(),
		strings.TrimSpace(C.GoString(localDataDir)),
		strings.TrimSpace(C.GoString(dstURI)),
		strings.TrimSpace(C.GoString(mode)),
		dryRun != 0,
	)
	if err != nil {
		*outErr = C.CString(err.Error())
		return 1
	}
	b, _ := json.Marshal(report)
	*outJSON = C.CString(string(b))
	return 0
}

//export RgRcloneExportPacksWithOpts
func RgRcloneExportPacksWithOpts(localDataDir *C.char, dstURI *C.char, mode *C.char, transfers C.int, checkers C.int, streams C.int, zstdLevel C.int, dryRun C.int, outJSON **C.char, outErr **C.char) C.int {
	ensureInitialized()
	report, err := vfsbridge.ConvertAndExportPacksWithOptions(
		context.Background(),
		vfsbridge.ExportOptions{
			LocalDataDir:       strings.TrimSpace(C.GoString(localDataDir)),
			DstURI:             strings.TrimSpace(C.GoString(dstURI)),
			Mode:               strings.TrimSpace(C.GoString(mode)),
			Transfers:          int(transfers),
			Checkers:           int(checkers),
			MultiThreadStreams: int(streams),
			ZstdLevel:          int(zstdLevel),
			DryRun:             dryRun != 0,
		},
	)
	if err != nil {
		*outErr = C.CString(err.Error())
		return 1
	}
	b, _ := json.Marshal(report)
	*outJSON = C.CString(string(b))
	return 0
}

func main() {}
