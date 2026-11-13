package team

import (
	"bytes"
	"encoding/json"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"
	"time"
	"unicode/utf16"
)

// Get walks decoded JSON as chained dict.get(key) calls: nil as soon as a
// step is missing or not an object.
func Get(v any, keys ...string) any {
	for _, k := range keys {
		m, ok := v.(map[string]any)
		if !ok {
			return nil
		}
		v = m[k]
	}
	return v
}

// Map is v as an object, or nil.
func Map(v any) map[string]any {
	m, _ := v.(map[string]any)
	return m
}

// List is v as an array, or nil.
func List(v any) []any {
	l, _ := v.([]any)
	return l
}

// Str is v as a string, or "".
func Str(v any) string {
	s, _ := v.(string)
	return s
}

// Int is v as an integer (JSON numbers decode as float64), or 0.
func Int(v any) int {
	switch n := v.(type) {
	case float64:
		return int(n)
	case int:
		return n
	case int64:
		return int(n)
	}
	return 0
}

// Repr formats decoded JSON as Python's str()/repr() of the same value
// does, for messages such as str(response["errors"]). Object keys are
// sorted: Go maps do not keep the server's order.
func Repr(v any) string {
	var b strings.Builder
	repr(&b, v)
	return b.String()
}

func repr(b *strings.Builder, v any) {
	switch x := v.(type) {
	case nil:
		b.WriteString("None")
	case bool:
		if x {
			b.WriteString("True")
		} else {
			b.WriteString("False")
		}
	case string:
		b.WriteString(pyQuote(x))
	case float64:
		if x == math.Trunc(x) && math.Abs(x) < 1e16 {
			b.WriteString(strconv.FormatInt(int64(x), 10))
		} else {
			b.WriteString(strconv.FormatFloat(x, 'g', -1, 64))
		}
	case int:
		b.WriteString(strconv.Itoa(x))
	case []any:
		b.WriteByte('[')
		for i, e := range x {
			if i > 0 {
				b.WriteString(", ")
			}
			repr(b, e)
		}
		b.WriteByte(']')
	case []string:
		b.WriteByte('[')
		for i, e := range x {
			if i > 0 {
				b.WriteString(", ")
			}
			b.WriteString(pyQuote(e))
		}
		b.WriteByte(']')
	case map[string]any:
		keys := make([]string, 0, len(x))
		for k := range x {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		b.WriteByte('{')
		for i, k := range keys {
			if i > 0 {
				b.WriteString(", ")
			}
			b.WriteString(pyQuote(k))
			b.WriteString(": ")
			repr(b, x[k])
		}
		b.WriteByte('}')
	default:
		fmt.Fprint(b, x)
	}
}

func pyQuote(s string) string {
	q := byte('\'')
	if strings.Contains(s, "'") && !strings.Contains(s, `"`) {
		q = '"'
	}
	var b strings.Builder
	b.WriteByte(q)
	for _, r := range s {
		switch {
		case r == '\\':
			b.WriteString(`\\`)
		case r == rune(q):
			b.WriteByte('\\')
			b.WriteRune(r)
		case r == '\n':
			b.WriteString(`\n`)
		case r == '\r':
			b.WriteString(`\r`)
		case r == '\t':
			b.WriteString(`\t`)
		case r < 0x20 || r == 0x7f:
			fmt.Fprintf(&b, `\x%02x`, r)
		default:
			b.WriteRune(r)
		}
	}
	b.WriteByte(q)
	return b.String()
}

// PyStr is Python's str(v) of a decoded JSON value: strings as they are,
// anything else as Repr.
func PyStr(v any) string {
	if s, ok := v.(string); ok {
		return s
	}
	return Repr(v)
}

// Truthy is Python's bool(v) of a decoded JSON value.
func Truthy(v any) bool {
	switch x := v.(type) {
	case nil:
		return false
	case bool:
		return x
	case string:
		return x != ""
	case float64:
		return x != 0
	case int:
		return x != 0
	case []any:
		return len(x) > 0
	case map[string]any:
		return len(x) > 0
	}
	return true
}

// DumpsIndent is json.dumps(v, indent=2) of the JSON text raw: its key order
// kept, non-ASCII escaped as \uXXXX (ensure_ascii).
func DumpsIndent(raw []byte) (string, error) {
	var buf bytes.Buffer
	if err := json.Indent(&buf, raw, "", "  "); err != nil {
		return "", err
	}
	var b strings.Builder
	for _, r := range buf.String() {
		switch {
		case r < 0x80:
			b.WriteRune(r)
		case r > 0xffff:
			r1, r2 := utf16.EncodeRune(r)
			fmt.Fprintf(&b, `\u%04x\u%04x`, r1, r2)
		default:
			fmt.Fprintf(&b, `\u%04x`, r)
		}
	}
	return b.String(), nil
}

// TimeLine is time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime(t)).
func TimeLine(t time.Time) string { return t.UTC().Format("2006-01-02 15:04:05") }
