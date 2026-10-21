package main

// Laid into Go taskloom's cmd/taskloom with `go test -overlay` by
// scripts/go-companion-fixtures.sh, so the Go checkout is not written. It
// dumps the companion surface Rust taskloom copies verbatim, straight from
// the Go source: the MCP server's instructions, its tools as tools/list
// answers them (each description, input and output schema), and the
// loadout document.

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestDumpCompanion(t *testing.T) {
	out := os.Getenv("GO_COMPANION_OUT")
	if out == "" {
		t.Skip("GO_COMPANION_OUT not set")
	}
	ctx := context.Background()
	clientT, serverT := mcp.NewInMemoryTransports()
	if _, err := newMCPServer().Connect(ctx, serverT, nil); err != nil {
		t.Fatal(err)
	}
	cs, err := mcp.NewClient(&mcp.Implementation{Name: "dump", Version: "0"}, nil).Connect(ctx, clientT, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cs.Close()
	tools, err := cs.ListTools(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := json.MarshalIndent(tools.Tools, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	files := map[string][]byte{
		"instructions.txt": []byte(mcpServerInstructions),
		"tools.json":       append(doc, '\n'),
		"loadout.yaml":     loadoutYAML,
	}
	for name, body := range files {
		if err := os.WriteFile(filepath.Join(out, name), body, 0o644); err != nil {
			t.Fatal(err)
		}
	}
}
