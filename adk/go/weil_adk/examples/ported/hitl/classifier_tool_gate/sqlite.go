package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"sync"
	"unicode"
)

// sqliteDB stands in for Python's sqlite3.connect(":memory:"). The module
// has no SQLite driver, so each statement runs through the sqlite3 command
// line shell against a private temporary database that is removed on Close:
// the same engine, so the same SQL dialect, results and error messages.
type sqliteDB struct {
	mu   sync.Mutex
	path string
}

// errSQL is a statement SQLite refused: Python's sqlite3.Error.
type errSQL struct{ msg string }

func (e *errSQL) Error() string { return e.msg }

func openSQLite(script string) (*sqliteDB, error) {
	if _, err := exec.LookPath("sqlite3"); err != nil {
		return nil, fmt.Errorf("the sqlite3 command line shell is required: %w", err)
	}
	f, err := os.CreateTemp("", "classifier_tool_gate-*.db")
	if err != nil {
		return nil, err
	}
	f.Close()
	db := &sqliteDB{path: f.Name()}
	if out, err := exec.Command("sqlite3", "-bail", "-batch", db.path, script).CombinedOutput(); err != nil {
		db.Close()
		return nil, fmt.Errorf("seed database: %v: %s", err, out)
	}
	return db, nil
}

// Close deletes the database.
func (db *sqliteDB) Close() error { return os.Remove(db.path) }

// Execute runs one statement and returns its rows and Python's
// cursor.rowcount: the rows an INSERT, UPDATE, DELETE or REPLACE changed,
// and -1 for any other statement.
func (db *sqliteDB) Execute(query string) (rows [][]any, rowCount int64, err error) {
	if hasMoreThanOneStatement(query) {
		return nil, 0, &errSQL{"You can only execute one statement at a time."}
	}
	db.mu.Lock()
	defer db.mu.Unlock()

	// The leading space keeps a statement starting with "." from being read
	// as a shell command; -safe refuses the dangerous ones regardless.
	cmd := exec.Command("sqlite3", "-bail", "-batch", "-safe", "-json", db.path,
		" "+query, "SELECT changes() AS n")
	var stdout, stderr bytes.Buffer
	cmd.Stdout, cmd.Stderr = &stdout, &stderr
	if err := cmd.Run(); err != nil {
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			return nil, 0, &errSQL{shellError(stderr.String())}
		}
		return nil, 0, err
	}

	// stdout holds the statement's rows (absent when it returned none), then
	// [{"n": changes}].
	var results [][][]any
	dec := json.NewDecoder(&stdout)
	dec.UseNumber()
	for {
		r, err := decodeRows(dec)
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, 0, fmt.Errorf("read sqlite3 output: %w", err)
		}
		results = append(results, r)
	}
	if len(results) == 0 {
		return nil, 0, errors.New("read sqlite3 output: no changes count")
	}
	changes := results[len(results)-1]
	rows = [][]any{}
	if len(results) > 1 {
		rows = results[0]
	}
	rowCount = -1
	if isDML(query) && len(changes) == 1 && len(changes[0]) == 1 {
		if n, ok := changes[0][0].(int64); ok {
			rowCount = n
		}
	}
	return rows, rowCount, nil
}

// decodeRows reads one JSON array of row objects, keeping each row's values
// in column order (as Python's list(row) does, duplicates included).
func decodeRows(dec *json.Decoder) ([][]any, error) {
	if _, err := dec.Token(); err != nil { // [
		return nil, err
	}
	var rows [][]any
	for dec.More() {
		if _, err := dec.Token(); err != nil { // {
			return nil, err
		}
		var row []any
		for dec.More() {
			if _, err := dec.Token(); err != nil { // column name
				return nil, err
			}
			var v any
			if err := dec.Decode(&v); err != nil {
				return nil, err
			}
			if n, ok := v.(json.Number); ok {
				if i, err := n.Int64(); err == nil {
					v = i
				} else if f, err := n.Float64(); err == nil {
					v = f
				}
			}
			row = append(row, v)
		}
		if _, err := dec.Token(); err != nil { // }
			return nil, err
		}
		rows = append(rows, row)
	}
	if _, err := dec.Token(); err != nil { // ]
		return nil, err
	}
	return rows, nil
}

var (
	shellErrorPrefix = regexp.MustCompile(`^(?:Error: )?(?:(?:Parse|Runtime) error near line \d+: |in prepare, |stepping, )`)
	shellErrorCode   = regexp.MustCompile(` \(\d+\)$`)
)

// shellError turns the shell's report, such as
// `Error: stepping, UNIQUE constraint failed: customers.id (19)`, into the
// message Python's sqlite3.Error carries: `UNIQUE constraint failed: customers.id`.
func shellError(stderr string) string {
	line, _, _ := strings.Cut(strings.TrimSpace(stderr), "\n")
	line = shellErrorPrefix.ReplaceAllString(line, "")
	return shellErrorCode.ReplaceAllString(line, "")
}

// isDML is CPython's sqlite3 check: after leading whitespace, the statement
// starts with INSERT, UPDATE, DELETE or REPLACE.
func isDML(query string) bool {
	q := strings.ToLower(strings.TrimLeft(query, " \t\r\n"))
	for _, kw := range []string{"insert", "update", "delete", "replace"} {
		if strings.HasPrefix(q, kw) {
			return true
		}
	}
	return false
}

// hasMoreThanOneStatement reports whether anything but whitespace and
// comments follows the first top-level ";", which Python's
// cursor.execute refuses. (It does not parse CREATE TRIGGER bodies.)
func hasMoreThanOneStatement(query string) bool {
	rest, ok := afterFirstStatement(query)
	if !ok {
		return false
	}
	for i := 0; i < len(rest); {
		switch {
		case unicode.IsSpace(rune(rest[i])):
			i++
		case strings.HasPrefix(rest[i:], "--"):
			end := strings.IndexByte(rest[i:], '\n')
			if end < 0 {
				return false
			}
			i += end + 1
		case strings.HasPrefix(rest[i:], "/*"):
			end := strings.Index(rest[i+2:], "*/")
			if end < 0 {
				return false
			}
			i += end + 4
		default:
			return true
		}
	}
	return false
}

// afterFirstStatement returns what follows the first ";" outside quotes,
// identifiers and comments.
func afterFirstStatement(q string) (string, bool) {
	for i := 0; i < len(q); i++ {
		switch c := q[i]; {
		case c == ';':
			return q[i+1:], true
		case c == '\'' || c == '"' || c == '`' || c == '[':
			closer := c
			if c == '[' {
				closer = ']'
			}
			for i++; i < len(q); i++ {
				if q[i] == closer {
					if closer != ']' && i+1 < len(q) && q[i+1] == closer {
						i++ // doubled quote
						continue
					}
					break
				}
			}
		case strings.HasPrefix(q[i:], "--"):
			end := strings.IndexByte(q[i:], '\n')
			if end < 0 {
				return "", false
			}
			i += end
		case strings.HasPrefix(q[i:], "/*"):
			end := strings.Index(q[i+2:], "*/")
			if end < 0 {
				return "", false
			}
			i += end + 3
		}
	}
	return "", false
}
