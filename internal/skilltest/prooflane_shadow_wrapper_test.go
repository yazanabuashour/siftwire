package skilltest_test

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestProoflaneShadowWrapperRunUsesSiblingAndPreservesStreams(t *testing.T) {
	t.Parallel()

	script, pathDir := prepareShadowWrapper(t, true)
	input := []byte("{\"action\":\"run_brief\",\"dry_run\":false}\nUTF-8: ✓\n")
	cmd := exec.Command(script, "run", "contracts/openbrief.json")
	cmd.Stdin = bytes.NewReader(input)
	cmd.Env = shadowEnv(pathDir, map[string]string{
		"OPENBRIEF_BINARY": "/tools/openbrief",
	})
	stdout, stderr, err := runShadowCommand(cmd)
	if err != nil {
		t.Fatalf("run wrapper: %v\nstderr: %s", err, stderr)
	}
	if !bytes.Equal(stdout, input) {
		t.Fatalf("stdout = %q, want exact stdin %q", stdout, input)
	}
	wantStderr := "source=sibling argv <dogfood> <openbrief> <run> <--contract> <contracts/openbrief.json> <--config-binary> </tools/openbrief> <--brief-binary> </tools/openbrief>\nprooflane-stderr\n"
	if string(stderr) != wantStderr {
		t.Fatalf("stderr = %q, want %q", stderr, wantStderr)
	}
}

func TestProoflaneShadowWrapperDeliveryUsesPathAndPreservesExit(t *testing.T) {
	t.Parallel()

	script, pathDir := prepareShadowWrapper(t, false)
	input := []byte("{\"action\":\"record_delivery\",\"run_id\":\"openbrief-run\",\"message\":\"NO_REPLY\"}\n")
	cmd := exec.Command(script, "delivery", "prooflane-run")
	cmd.Stdin = bytes.NewReader(input)
	cmd.Env = shadowEnv(pathDir, map[string]string{
		"OPENBRIEF_BINARY": "/tools/openbrief",
		"STUB_EXIT_CODE":   "17",
	})
	stdout, stderr, err := runShadowCommand(cmd)
	exitError, ok := err.(*exec.ExitError)
	if !ok || exitError.ExitCode() != 17 {
		t.Fatalf("exit error = %v, want status 17", err)
	}
	if !bytes.Equal(stdout, input) {
		t.Fatalf("stdout = %q, want exact stdin %q", stdout, input)
	}
	wantStderr := "source=path argv <dogfood> <openbrief> <delivery> <--run> <prooflane-run> <--brief-binary> </tools/openbrief>\nprooflane-stderr\n"
	if string(stderr) != wantStderr {
		t.Fatalf("stderr = %q, want %q", stderr, wantStderr)
	}
}

func TestProoflaneShadowWrapperRejectsInvalidArguments(t *testing.T) {
	t.Parallel()

	script, pathDir := prepareShadowWrapper(t, false)
	wantUsage := "usage: scripts/prooflane-shadow-openbrief.sh run <contract>\n" +
		"       scripts/prooflane-shadow-openbrief.sh delivery <prooflane-run-id>\n"
	for _, args := range [][]string{
		nil,
		{"run"},
		{"run", "contract", "extra"},
		{"unknown", "value"},
	} {
		cmd := exec.Command(script, args...)
		cmd.Env = shadowEnv(pathDir, nil)
		stdout, stderr, err := runShadowCommand(cmd)
		exitError, ok := err.(*exec.ExitError)
		if !ok || exitError.ExitCode() != 2 {
			t.Fatalf("args %v exit error = %v, want status 2", args, err)
		}
		if len(stdout) != 0 || string(stderr) != wantUsage {
			t.Fatalf("args %v stdout=%q stderr=%q", args, stdout, stderr)
		}
	}
}

func prepareShadowWrapper(t *testing.T, withSibling bool) (string, string) {
	t.Helper()
	root := t.TempDir()
	scriptDir := filepath.Join(root, "openbrief", "scripts")
	if err := os.MkdirAll(scriptDir, 0o755); err != nil {
		t.Fatalf("create script directory: %v", err)
	}
	script := filepath.Join(scriptDir, "prooflane-shadow-openbrief.sh")
	if err := os.WriteFile(script, readShadowWrapper(t), 0o755); err != nil {
		t.Fatalf("copy wrapper: %v", err)
	}
	if withSibling {
		writeProoflaneStub(t, filepath.Join(root, "prooflane", "bin", "prooflane"), "sibling")
	}
	pathDir := filepath.Join(root, "path-bin")
	writeProoflaneStub(t, filepath.Join(pathDir, "prooflane"), "path")
	return script, pathDir
}

func readShadowWrapper(t *testing.T) []byte {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve wrapper test path")
	}
	path := filepath.Join(filepath.Dir(file), "..", "..", "scripts", "prooflane-shadow-openbrief.sh")
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read wrapper: %v", err)
	}
	return content
}

func writeProoflaneStub(t *testing.T, path string, source string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("create stub directory: %v", err)
	}
	stub := fmt.Sprintf(`#!/bin/sh
{
	printf 'source=%s argv'
	for argument in "$@"; do
		printf ' <%%s>' "$argument"
	done
	printf '\nprooflane-stderr\n'
} >&2
cat
exit "${STUB_EXIT_CODE:-0}"
`, source)
	if err := os.WriteFile(path, []byte(stub), 0o755); err != nil {
		t.Fatalf("write stub: %v", err)
	}
}

func shadowEnv(pathDir string, values map[string]string) []string {
	blocked := map[string]bool{
		"OPENBRIEF_BINARY": true,
		"PROOFLANE_BINARY": true,
		"STUB_EXIT_CODE":   true,
		"PATH":             true,
	}
	environment := make([]string, 0, len(os.Environ())+len(values)+1)
	for _, entry := range os.Environ() {
		key, _, _ := strings.Cut(entry, "=")
		if !blocked[key] {
			environment = append(environment, entry)
		}
	}
	environment = append(environment, "PATH="+pathDir+string(os.PathListSeparator)+os.Getenv("PATH"))
	for key, value := range values {
		environment = append(environment, key+"="+value)
	}
	return environment
}

func runShadowCommand(cmd *exec.Cmd) ([]byte, []byte, error) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	return stdout.Bytes(), stderr.Bytes(), err
}
