package main

// Laid into Go taskloom's cmd/taskloom with `go test -overlay` by
// scripts/go-companion-fixtures.sh, so the Go checkout is not written. It
// dumps the companion surface Rust taskloom copies, straight from
// the Go source: the MCP server's instructions, its capabilities, its
// tools as tools/list answers them (each description, input and output
// schema), its resources as resources/list answers them, what each
// resource reads (the tag schema with nothing configured; the vocabulary
// of a seeded project, its id and directory left as PROJECT_ID and
// PROJECT_DIR), and the loadout document.

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestDumpCompanion(t *testing.T) {
	out := os.Getenv("GO_COMPANION_OUT")
	if out == "" {
		t.Skip("GO_COMPANION_OUT not set")
	}
	withProjectDir(t)
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
	indented := func(v any) []byte {
		doc, err := json.MarshalIndent(v, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		return append(doc, '\n')
	}
	tools, err := cs.ListTools(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	resources, err := cs.ListResources(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	read := func(uri string) string {
		res, err := cs.ReadResource(ctx, &mcp.ReadResourceParams{URI: uri})
		if err != nil {
			t.Fatal(err)
		}
		return res.Contents[0].Text
	}
	// Read before anything is added: the project's schema is the default.
	schema := read(tagSchemaResourceURI)

	add := func(text, status, trigger string, tags ...string) {
		in := taskAddInput{Text: text, Status: status, Trigger: trigger, Tags: tags}
		if _, _, err := handleTaskAdd(ctx, nil, in); err != nil {
			t.Fatal(err)
		}
	}
	add("first", "", "", "urgent", "triage:kind=defect")
	add("second", "Done", "", "urgent", "release")
	add("third", "Deferred", "the v2 API ships", "release")
	add("fourth", "In Progress", "", "version=1.2.3", "urgent")
	vocabulary := read(tagVocabularyResourceURI)
	var placed tagVocabularyDoc
	if err := json.Unmarshal([]byte(vocabulary), &placed); err != nil {
		t.Fatal(err)
	}
	if placed.ProjectID == "" || placed.ProjectDir == "" {
		t.Fatalf("the vocabulary names no project: %s", vocabulary)
	}
	vocabulary = strings.ReplaceAll(vocabulary, placed.ProjectDir, "PROJECT_DIR")
	vocabulary = strings.ReplaceAll(vocabulary, placed.ProjectID, "PROJECT_ID")

	files := map[string][]byte{
		"instructions.txt":    []byte(mcpServerInstructions),
		"capabilities.json":   indented(cs.InitializeResult().Capabilities),
		"tools.json":          indented(tools.Tools),
		"resources.json":      indented(resources.Resources),
		"tag-schema.json":     []byte(schema),
		"tag-vocabulary.json": []byte(vocabulary),
		"loadout.yaml":        loadoutYAML,
	}
	for name, body := range files {
		if err := os.WriteFile(filepath.Join(out, name), body, 0o644); err != nil {
			t.Fatal(err)
		}
	}
}
