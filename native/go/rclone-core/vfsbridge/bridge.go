// SPDX-License-Identifier: Apache-2.0

package vfsbridge

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"slices"
	"strings"
	"sync"

	"github.com/rclone/rclone/backend/nievfs"
	"github.com/rclone/rclone/fs"
	"github.com/rclone/rclone/fs/cache"
	"github.com/rclone/rclone/fs/config"
)

const maxRemoteReadBytes = 256 * 1024 * 1024

var fsCacheMu sync.Mutex

func rejectUnsafeURI(u *url.URL) error {
	if u.User != nil {
		if _, hasPass := u.User.Password(); hasPass {
			return errors.New("URI password is forbidden; use rclone.conf, env vars, or SSH config")
		}
		if (u.Scheme == "http" || u.Scheme == "https") && u.User.Username() != "" {
			return errors.New("HTTP URI credentials are forbidden; use NIE_HTTP_AUTH_TOKEN")
		}
	}
	if u.RawQuery != "" || u.Fragment != "" {
		return errors.New("URI query/fragment is forbidden for remote VFS sources")
	}
	return nil
}

func sshAliasConfig(alias string) (host, user, port, keyFile string) {
	home, _ := os.UserHomeDir()
	cfgPath := filepath.Join(home, ".ssh", "config")
	f, err := os.Open(cfgPath)
	if err != nil {
		return alias, "", "", ""
	}
	defer func() { _ = f.Close() }()

	inBlock := false
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) < 2 {
			continue
		}
		k := strings.ToLower(fields[0])
		v := strings.Join(fields[1:], " ")
		if k == "host" {
			inBlock = slices.Contains(fields[1:], alias)
			continue
		}
		if !inBlock {
			continue
		}
		switch k {
		case "hostname":
			host = v
		case "user":
			user = v
		case "port":
			port = v
		case "identityfile":
			if strings.HasPrefix(v, "~/") {
				v = filepath.Join(home, v[2:])
			}
			keyFile = v
		}
	}
	if host == "" {
		host = alias
	}
	return host, user, port, keyFile
}

func quoteOpt(v string) string {
	v = strings.ReplaceAll(v, `"`, `\"`)
	return `"` + v + `"`
}

func hasConfiguredRemote(name string) bool {
	for _, r := range config.GetRemotes() {
		if r.Name == name {
			return true
		}
	}
	return false
}

// ResolveSpec translates rclone://, r2://, s3://, gdrive://, ssh://, sftp://,
// http://, https://, nievfs:// URIs or rclone remote:path specs into an rclone Fs string.
func ResolveSpec(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", errors.New("empty remote URI")
	}
	if !strings.Contains(raw, "://") {
		return raw, nil
	}
	u, err := url.Parse(raw)
	if err != nil {
		return "", fmt.Errorf("invalid URI %q: %w", raw, err)
	}
	if err := rejectUnsafeURI(u); err != nil {
		return "", err
	}

	cleanPath := strings.TrimPrefix(u.Path, "/")
	cleanPath = strings.TrimSuffix(cleanPath, "/nie.exe")
	cleanPath = strings.TrimSuffix(cleanPath, "/")

	switch u.Scheme {
	case "rclone":
		remoteName := u.Host
		if remoteName == "" {
			return "", errors.New("rclone:// URI requires remote host name, e.g. rclone://r2/bucket/data")
		}
		return remoteName + ":" + cleanPath, nil

	case "nievfs":
		if u.Host == "" {
			return ":nievfs:" + cleanPath, nil
		}
		if hasConfiguredRemote(u.Host) {
			return ":nievfs:" + u.Host + ":" + cleanPath, nil
		}
		if cleanPath == "" {
			return ":nievfs:" + u.Host, nil
		}
		return ":nievfs:" + u.Host + "/" + cleanPath, nil

	case "r2":
		if u.Host == "" {
			return "", errors.New("r2:// URI requires bucket name, e.g. r2://rosegriffon/ievr")
		}
		bucketAndPath := u.Host
		if cleanPath != "" {
			bucketAndPath += "/" + cleanPath
		}
		if hasConfiguredRemote("r2") && os.Getenv("R2_ACCESS_KEY_ID") == "" {
			return "r2:" + bucketAndPath, nil
		}
		ak := os.Getenv("R2_ACCESS_KEY_ID")
		sk := os.Getenv("R2_SECRET_ACCESS_KEY")
		ep := os.Getenv("R2_ENDPOINT")
		if ep == "" {
			if acct := os.Getenv("R2_ACCOUNT_ID"); acct != "" {
				ep = fmt.Sprintf("https://%s.r2.cloudflarestorage.com", acct)
			}
		}
		if ak != "" && sk != "" && ep != "" {
			return fmt.Sprintf(":s3,provider=Cloudflare,access_key_id=%s,secret_access_key=%s,endpoint=%s,region=auto:%s",
				quoteOpt(ak), quoteOpt(sk), quoteOpt(ep), bucketAndPath), nil
		}
		if hasConfiguredRemote("r2") {
			return "r2:" + bucketAndPath, nil
		}
		return "", errors.New("R2 credentials missing (configure rclone 'r2' remote or R2_ACCESS_KEY_ID/R2_SECRET_ACCESS_KEY/R2_ACCOUNT_ID)")

	case "s3":
		if u.Host == "" {
			return "", errors.New("s3:// URI requires bucket name")
		}
		bucketAndPath := u.Host
		if cleanPath != "" {
			bucketAndPath += "/" + cleanPath
		}
		if ak, sk := os.Getenv("S3_ACCESS_KEY_ID"), os.Getenv("S3_SECRET_ACCESS_KEY"); ak != "" && sk != "" {
			opts := []string{
				"provider=AWS",
				"access_key_id=" + quoteOpt(ak),
				"secret_access_key=" + quoteOpt(sk),
			}
			if ep := os.Getenv("S3_ENDPOINT"); ep != "" {
				opts = append(opts, "endpoint="+quoteOpt(ep))
			}
			if reg := os.Getenv("S3_REGION"); reg != "" {
				opts = append(opts, "region="+quoteOpt(reg))
			}
			return ":" + "s3," + strings.Join(opts, ",") + ":" + bucketAndPath, nil
		}
		if ak, sk := os.Getenv("AWS_ACCESS_KEY_ID"), os.Getenv("AWS_SECRET_ACCESS_KEY"); ak != "" && sk != "" {
			opts := []string{
				"provider=AWS",
				"access_key_id=" + quoteOpt(ak),
				"secret_access_key=" + quoteOpt(sk),
			}
			if reg := os.Getenv("AWS_REGION"); reg != "" {
				opts = append(opts, "region="+quoteOpt(reg))
			}
			return ":s3," + strings.Join(opts, ",") + ":" + bucketAndPath, nil
		}
		if hasConfiguredRemote("s3") {
			return "s3:" + bucketAndPath, nil
		}
		return "", errors.New("S3 credentials missing (configure rclone 's3' remote or S3_*/AWS_* env vars)")

	case "gdrive":
		remoteName := u.Host
		if remoteName == "" {
			if hasConfiguredRemote("gdrive-ai") {
				remoteName = "gdrive-ai"
			} else {
				remoteName = "gdrive"
			}
		}
		return remoteName + ":" + cleanPath, nil

	case "ssh", "sftp":
		alias := u.Host
		if alias == "" {
			return "", errors.New("ssh/sftp URI requires host")
		}
		host, user, port, keyFile := sshAliasConfig(alias)
		if u.User != nil && u.User.Username() != "" {
			user = u.User.Username()
		}
		if p := u.Port(); p != "" {
			port = p
		}
		opts := []string{"host=" + quoteOpt(host)}
		if user != "" {
			opts = append(opts, "user="+quoteOpt(user))
		}
		if port != "" {
			opts = append(opts, "port="+quoteOpt(port))
		}
		if keyFile != "" {
			opts = append(opts, "key_file="+quoteOpt(keyFile))
		}
		return ":sftp," + strings.Join(opts, ",") + ":" + cleanPath, nil

	case "http", "https":
		base := fmt.Sprintf("%s://%s", u.Scheme, u.Host)
		if u.Path != "" && u.Path != "/" {
			p := strings.TrimSuffix(u.Path, "/nie.exe")
			base += strings.TrimSuffix(p, "/") + "/"
		} else {
			base += "/"
		}
		opts := []string{"url=" + quoteOpt(base)}
		if tok := os.Getenv("NIE_HTTP_AUTH_TOKEN"); tok != "" {
			opts = append(opts, "headers="+quoteOpt("Authorization,Bearer "+tok))
		}
		return ":http," + strings.Join(opts, ",") + ":", nil

	default:
		return "", fmt.Errorf("unsupported remote URI scheme %q", u.Scheme)
	}
}

func openFs(ctx context.Context, spec string) (fs.Fs, error) {
	resolved, err := ResolveSpec(spec)
	if err != nil {
		return nil, err
	}
	fsCacheMu.Lock()
	defer fsCacheMu.Unlock()
	return cache.Get(ctx, resolved)
}

// ReadRemoteRange reads [offset, offset+length) or the full file (when length < 0)
// cleanly through any rclone remote backend.
func ReadRemoteRange(ctx context.Context, spec, relPath string, offset, length int64) ([]byte, error) {
	if length > maxRemoteReadBytes || (length >= 0 && offset+length < offset) {
		return nil, errors.New("remote read exceeds 256 MiB budget")
	}
	f, err := openFs(ctx, spec)
	if err != nil {
		return nil, err
	}
	rel := strings.TrimPrefix(nievfs.CanonicalPath(relPath), "/")
	obj, err := f.NewObject(ctx, rel)
	if err != nil {
		return nil, err
	}
	if length == 0 {
		return nil, nil
	}
	var opts []fs.OpenOption
	if length > 0 {
		opts = append(opts, &fs.RangeOption{Start: offset, End: offset + length - 1})
	} else if offset > 0 {
		opts = append(opts, &fs.SeekOption{Offset: offset})
	}
	rc, err := obj.Open(ctx, opts...)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rc.Close() }()

	if length > 0 {
		buf := make([]byte, length)
		n, err := io.ReadFull(rc, buf)
		if err != nil {
			return nil, err
		}
		return buf[:n], nil
	}
	limited := io.LimitReader(rc, maxRemoteReadBytes+1)
	data, err := io.ReadAll(limited)
	if err != nil {
		return nil, err
	}
	if len(data) > maxRemoteReadBytes {
		return nil, errors.New("remote object exceeds 256 MiB budget")
	}
	return data, nil
}

// ListRemoteDir lists one directory level, appending "/" to directory names.
func ListRemoteDir(ctx context.Context, spec, relPath string) ([]string, error) {
	f, err := openFs(ctx, spec)
	if err != nil {
		return nil, err
	}
	dir := strings.Trim(nievfs.CanonicalPath(relPath), "/")
	entries, err := f.List(ctx, dir)
	if err != nil {
		return nil, err
	}
	names := make([]string, 0, len(entries))
	for _, e := range entries {
		leaf := path.Base(strings.TrimSuffix(e.Remote(), "/"))
		if leaf == "" || leaf == "." {
			continue
		}
		if _, isDir := e.(fs.Directory); isDir {
			names = append(names, leaf+"/")
		} else {
			names = append(names, leaf)
		}
	}
	slices.Sort(names)
	return slices.Compact(names), nil
}

// PackExportReport describes cloud conversion / sync result.
type PackExportReport struct {
	Mode             string   `json:"mode"`
	Source           string   `json:"source"`
	Destination      string   `json:"destination"`
	ResolvedDst      string   `json:"resolved_destination"`
	PackCount        int      `json:"pack_count"`
	LogicalCount     int      `json:"logical_count,omitempty"`
	CopiedObjects    int      `json:"copied_objects"`
	SkippedObjects   int      `json:"skipped_objects,omitempty"`
	TotalSourceBytes int64    `json:"total_source_bytes,omitempty"`
	TotalOutputBytes int64    `json:"total_output_bytes,omitempty"`
	Transfers        int      `json:"transfers,omitempty"`
	DurationMs       int64    `json:"duration_ms,omitempty"`
	ManifestPath     string   `json:"manifest_path"`
	ManifestHash     string   `json:"manifest_sha256"`
	SamplePacks      []string `json:"sample_packs,omitempty"`
}

// ConvertAndExportPacks prepares IEVR data/packs for R2 / S3 / Google Drive / rclone remotes.
func ConvertAndExportPacks(ctx context.Context, localDataDir, dstURI, mode string, dryRun bool) (*PackExportReport, error) {
	return ConvertAndExportPacksWithOptions(ctx, ExportOptions{
		LocalDataDir: localDataDir,
		DstURI:       dstURI,
		Mode:         mode,
		DryRun:       dryRun,
	})
}
