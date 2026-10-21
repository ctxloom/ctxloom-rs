// Go taskloom's tag queries over a set of cases, dumped: the oracle
// `taskloom list --tag-query` is compared against. Not part of either
// build: scripts/compare-go-tag-queries.sh lays this file into Go's
// internal/shared/tasks package with `go test -overlay`, so the Go
// checkout is only read.
//
// GO_TAGQUERY_CASES names the cases (cases.tsv: tab-separated `scenario`,
// `schema`, `status`, `task` and `query` lines); GO_TAGQUERY_OUT the file
// written, one tab-separated line per query in the cases' order:
// `<scenario> <query> ok <ids, sorted, comma-separated>` or
// `<scenario> <query> error <message>`.
package tasks

import (
	"bufio"
	"fmt"
	"os"
	"sort"
	"strings"
	"testing"

	"github.com/ctxloom/ctxloom/internal/shared/tasks/tagschema"
)

type queryScenario struct {
	name     string
	schema   []string
	statuses []string
	tasks    []Task
	queries  []string
}

func readQueryCases(t *testing.T, path string) []*queryScenario {
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var scenarios []*queryScenario
	lines := bufio.NewScanner(f)
	for lines.Scan() {
		line := lines.Text()
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		fields := strings.Split(line, "\t")
		if fields[0] == "scenario" {
			scenarios = append(scenarios, &queryScenario{name: fields[1]})
			continue
		}
		s := scenarios[len(scenarios)-1]
		switch fields[0] {
		case "schema":
			s.schema = append(s.schema, fields[1])
		case "status":
			s.statuses = append(s.statuses, fields[1])
		case "task":
			s.tasks = append(s.tasks, Task{HarpID: fields[1], Text: fields[1], Status: fields[2], Tags: fields[3:]})
		case "query":
			q := ""
			if len(fields) > 1 {
				q = fields[1]
			}
			s.queries = append(s.queries, q)
		default:
			t.Fatalf("unknown line %q", line)
		}
	}
	if err := lines.Err(); err != nil {
		t.Fatal(err)
	}
	return scenarios
}

func TestDumpTagQueries(t *testing.T) {
	in, out := os.Getenv("GO_TAGQUERY_CASES"), os.Getenv("GO_TAGQUERY_OUT")
	if in == "" || out == "" {
		t.Skip("GO_TAGQUERY_CASES and GO_TAGQUERY_OUT name the cases and the dump")
	}
	var dump strings.Builder
	for _, s := range readQueryCases(t, in) {
		var schema *tagschema.Schema
		if len(s.schema) > 0 {
			var err error
			if schema, err = tagschema.Parse(s.schema); err != nil {
				t.Fatal(err)
			}
		}
		for _, q := range s.queries {
			got, err := filterTasks(s.tasks, s.statuses, "", q, schema)
			if err != nil {
				fmt.Fprintf(&dump, "%s\t%s\terror\t%s\n", s.name, q, strings.ReplaceAll(err.Error(), "\n", " "))
				continue
			}
			ids := []string{}
			for _, task := range got {
				ids = append(ids, task.HarpID)
			}
			sort.Strings(ids)
			fmt.Fprintf(&dump, "%s\t%s\tok\t%s\n", s.name, q, strings.Join(ids, ","))
		}
	}
	if err := os.WriteFile(out, []byte(dump.String()), 0o644); err != nil {
		t.Fatal(err)
	}
}
