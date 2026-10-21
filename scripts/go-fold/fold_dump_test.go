// Go taskloom's fold of every log in a directory, dumped as JSON: the oracle
// `taskloom import` is compared against. Not part of either build: the
// comparison script lays this file into Go's internal/shared/tasks package
// with `go test -overlay`, so the Go checkout is only read.
//
// GO_FOLD_LOGS names the directory of <project-id>.jsonl logs; GO_FOLD_OUT
// the JSON file written.
package tasks

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"
)

type dumpedTask struct {
	HarpID        string     `json:"harp_id"`
	Removed       bool       `json:"removed"`
	Text          string     `json:"text"`
	Status        string     `json:"status"`
	Trigger       string     `json:"trigger"`
	OriginSession string     `json:"origin_session"`
	Tags          []string   `json:"tags"`
	CreatedAt     *time.Time `json:"created_at"`
	StatusSetAt   *time.Time `json:"status_set_at"`
	DeferredSince *time.Time `json:"deferred_since"`
}

type dumpedAnomaly struct {
	HarpID   string `json:"harp_id"`
	Text     string `json:"text"`
	Session  string `json:"session"`
	Key      string `json:"key"`
	Repaired bool   `json:"repaired"`
}

type dumpedProject struct {
	Error     string          `json:"error,omitempty"`
	Tasks     []dumpedTask    `json:"tasks"`
	Anomalies []dumpedAnomaly `json:"anomalies"`
}

func at(t time.Time) *time.Time { u := t.UTC(); return &u }

func TestDumpFold(t *testing.T) {
	dir, out := os.Getenv("GO_FOLD_LOGS"), os.Getenv("GO_FOLD_OUT")
	if dir == "" || out == "" {
		t.Skip("GO_FOLD_LOGS and GO_FOLD_OUT name the logs and the dump")
	}
	paths, err := filepath.Glob(filepath.Join(dir, "*.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	dump := map[string]dumpedProject{}
	for _, p := range paths {
		id := strings.TrimSuffix(filepath.Base(p), ".jsonl")
		f, err := (&eventLog{path: p}).fold()
		if err != nil {
			// The log's directory is left out, so a dump names no machine's paths.
			msg := strings.ReplaceAll(err.Error(), dir+string(filepath.Separator), "")
			dump[id] = dumpedProject{Error: msg, Tasks: []dumpedTask{}, Anomalies: []dumpedAnomaly{}}
			continue
		}
		proj := dumpedProject{Tasks: []dumpedTask{}, Anomalies: []dumpedAnomaly{}}
		harps := make([]string, 0, len(f.issued))
		for h := range f.issued {
			harps = append(harps, h)
		}
		sort.Strings(harps)
		for _, h := range harps {
			task := f.byID[h]
			if task == nil {
				proj.Tasks = append(proj.Tasks, dumpedTask{HarpID: h, Removed: true, Tags: []string{}})
				continue
			}
			d := dumpedTask{
				HarpID: h, Text: task.Text, Status: task.Status, Trigger: task.Trigger,
				OriginSession: task.OriginSession, Tags: task.Tags, CreatedAt: at(task.CreatedAt),
			}
			if d.Tags == nil {
				d.Tags = []string{}
			}
			if ts, ok := f.statusSetAt[h]; ok {
				d.StatusSetAt = at(ts)
			}
			if ts, ok := f.deferredSince[h]; ok {
				d.DeferredSince = at(ts)
			}
			proj.Tasks = append(proj.Tasks, d)
		}
		for _, a := range f.anomalies {
			key := anomalyKey(a)
			_, done := f.repaired[key]
			proj.Anomalies = append(proj.Anomalies, dumpedAnomaly{HarpID: a.Task, Text: a.Text, Session: a.Session, Key: key, Repaired: done})
		}
		dump[id] = proj
	}
	b, err := json.MarshalIndent(dump, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(out, append(b, '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
}
