package client

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

// PythonJSON encodes v exactly as the Python SDK canonicalizes request
// bodies — json.dumps(v, separators=(",", ":"), sort_keys=True) — so a body
// hash computed in Go matches one computed in Python byte for byte:
//
//   - object keys sorted, no whitespace;
//   - non-ASCII escaped as \uXXXX (surrogate pairs above U+FFFF), as with
//     Python's default ensure_ascii;
//   - none of encoding/json's HTML escaping of <, > and &;
//   - floats printed like Python's repr (1.0, 1e+16), integers verbatim.
//
// v may be any value encoding/json can marshal; it is normalized through a
// JSON round trip first.
func PythonJSON(v any) ([]byte, error) {
	raw, err := json.Marshal(v)
	if err != nil {
		return nil, err
	}
	return PythonJSONFromRaw(raw)
}

// PythonJSONFromRaw re-encodes a JSON document the way PythonJSON does, as
// Python's json.loads followed by the canonical json.dumps would.
func PythonJSONFromRaw(raw []byte) ([]byte, error) {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	var v any
	if err := dec.Decode(&v); err != nil {
		return nil, err
	}
	var buf bytes.Buffer
	if err := writePython(&buf, v, canonicalStyle); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// pyStyle is how a Python json.dumps call lays out its output.
type pyStyle struct {
	itemSep, keySep string
	sortKeys        bool
}

var (
	// json.dumps(v, separators=(",", ":"), sort_keys=True)
	canonicalStyle = pyStyle{itemSep: ",", keySep: ":", sortKeys: true}
	// json.dumps(v, sort_keys=True)
	sortedStyle = pyStyle{itemSep: ", ", keySep: ": ", sortKeys: true}
	// json.dumps(v)
	defaultStyle = pyStyle{itemSep: ", ", keySep: ": "}
)

// PythonDumps encodes the JSON document raw as Python's json.dumps(v) or,
// with sortKeys, json.dumps(v, sort_keys=True) would after json.loads: ", "
// and ": " separators, non-ASCII escaped, no HTML escaping, and object keys
// in their order in raw unless sorted.
func PythonDumps(raw []byte, sortKeys bool) ([]byte, error) {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	v, err := decodeOrdered(dec)
	if err != nil {
		return nil, err
	}
	style := defaultStyle
	if sortKeys {
		style = sortedStyle
	}
	var buf bytes.Buffer
	if err := writePython(&buf, v, style); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// PythonDumpsValue is PythonDumps for any value encoding/json can marshal.
// Go maps have no order, so pass sortKeys or an ordered source (a struct, or
// raw JSON) to get Python's insertion order.
func PythonDumpsValue(v any, sortKeys bool) ([]byte, error) {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(v); err != nil {
		return nil, err
	}
	return PythonDumps(buf.Bytes(), sortKeys)
}

// orderedObject is a JSON object with its keys in document order.
type orderedObject []orderedMember

type orderedMember struct {
	key   string
	value any
}

func decodeOrdered(dec *json.Decoder) (any, error) {
	tok, err := dec.Token()
	if err != nil {
		return nil, err
	}
	switch t := tok.(type) {
	case json.Delim:
		switch t {
		case '{':
			obj := orderedObject{}
			for dec.More() {
				kt, err := dec.Token()
				if err != nil {
					return nil, err
				}
				val, err := decodeOrdered(dec)
				if err != nil {
					return nil, err
				}
				key := kt.(string)
				// A repeated key keeps its first position and last value,
				// as a Python dict does.
				replaced := false
				for i := range obj {
					if obj[i].key == key {
						obj[i].value, replaced = val, true
						break
					}
				}
				if !replaced {
					obj = append(obj, orderedMember{key, val})
				}
			}
			_, err := dec.Token()
			return obj, err
		case '[':
			arr := []any{}
			for dec.More() {
				val, err := decodeOrdered(dec)
				if err != nil {
					return nil, err
				}
				arr = append(arr, val)
			}
			_, err := dec.Token()
			return arr, err
		}
	}
	return tok, nil
}

// BodySHA256 is the hex SHA-256 the auth headers bind a request body with
// (X-Body-SHA256): raw bytes and strings are hashed as they are, anything else
// over its PythonJSON encoding.
func BodySHA256(body any) (string, error) {
	var b []byte
	switch v := body.(type) {
	case []byte:
		b = v
	case string:
		b = []byte(v)
	case json.RawMessage:
		var err error
		if b, err = PythonJSONFromRaw(v); err != nil {
			return "", err
		}
	default:
		var err error
		if b, err = PythonJSON(v); err != nil {
			return "", err
		}
	}
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:]), nil
}

func writePython(buf *bytes.Buffer, v any, st pyStyle) error {
	switch x := v.(type) {
	case nil:
		buf.WriteString("null")
	case bool:
		if x {
			buf.WriteString("true")
		} else {
			buf.WriteString("false")
		}
	case string:
		writePythonString(buf, x)
	case json.Number:
		s, err := pythonNumber(string(x))
		if err != nil {
			return err
		}
		buf.WriteString(s)
	case []any:
		buf.WriteByte('[')
		for i, e := range x {
			if i > 0 {
				buf.WriteString(st.itemSep)
			}
			if err := writePython(buf, e, st); err != nil {
				return err
			}
		}
		buf.WriteByte(']')
	case map[string]any:
		obj := make(orderedObject, 0, len(x))
		for k, val := range x {
			obj = append(obj, orderedMember{k, val})
		}
		sort.Slice(obj, func(i, j int) bool { return obj[i].key < obj[j].key })
		return writePython(buf, obj, st)
	case orderedObject:
		if st.sortKeys {
			// Byte order of UTF-8 equals code point order, which is how
			// Python sorts str keys.
			x = append(orderedObject(nil), x...)
			sort.SliceStable(x, func(i, j int) bool { return x[i].key < x[j].key })
		}
		buf.WriteByte('{')
		for i, m := range x {
			if i > 0 {
				buf.WriteString(st.itemSep)
			}
			writePythonString(buf, m.key)
			buf.WriteString(st.keySep)
			if err := writePython(buf, m.value, st); err != nil {
				return err
			}
		}
		buf.WriteByte('}')
	default:
		return fmt.Errorf("pythonjson: unsupported type %T", v)
	}
	return nil
}

func writePythonString(buf *bytes.Buffer, s string) {
	const hexDigits = "0123456789abcdef"
	buf.WriteByte('"')
	for _, r := range s {
		switch r {
		case '"':
			buf.WriteString(`\"`)
		case '\\':
			buf.WriteString(`\\`)
		case '\n':
			buf.WriteString(`\n`)
		case '\r':
			buf.WriteString(`\r`)
		case '\t':
			buf.WriteString(`\t`)
		case '\b':
			buf.WriteString(`\b`)
		case '\f':
			buf.WriteString(`\f`)
		default:
			switch {
			case r < 0x20 || (r >= 0x7f && r <= 0xffff) || r == utf8.RuneError:
				u := uint16(r)
				buf.WriteString(`\u`)
				buf.WriteByte(hexDigits[u>>12&0xf])
				buf.WriteByte(hexDigits[u>>8&0xf])
				buf.WriteByte(hexDigits[u>>4&0xf])
				buf.WriteByte(hexDigits[u&0xf])
			case r > 0xffff:
				r -= 0x10000
				for _, u := range []uint16{uint16(0xd800 + (r >> 10)), uint16(0xdc00 + (r & 0x3ff))} {
					buf.WriteString(`\u`)
					buf.WriteByte(hexDigits[u>>12&0xf])
					buf.WriteByte(hexDigits[u>>8&0xf])
					buf.WriteByte(hexDigits[u>>4&0xf])
					buf.WriteByte(hexDigits[u&0xf])
				}
			default:
				buf.WriteRune(r)
			}
		}
	}
	buf.WriteByte('"')
}

// pythonNumber formats a JSON number literal as Python would after
// json.loads: integers keep every digit (Python ints are unbounded), floats
// use repr.
func pythonNumber(lit string) (string, error) {
	if !strings.ContainsAny(lit, ".eE") {
		if strings.HasPrefix(lit, "-0") && len(lit) == 2 {
			return "0", nil
		}
		return lit, nil
	}
	f, err := strconv.ParseFloat(lit, 64)
	if err != nil {
		return "", err
	}
	return pythonFloatRepr(f), nil
}

// pythonFloatRepr mirrors Python's float.__repr__: the shortest round-trip
// digits, in positional notation for decimal exponents in [-4, 16), and
// otherwise as d.ddde±XX.
func pythonFloatRepr(f float64) string {
	switch {
	case math.IsInf(f, 1):
		return "Infinity"
	case math.IsInf(f, -1):
		return "-Infinity"
	case math.IsNaN(f):
		return "NaN"
	}
	sign := ""
	if math.Signbit(f) {
		sign = "-"
		f = -f
	}
	if f == 0 {
		return sign + "0.0"
	}
	// 'e' with precision -1 gives the shortest digits: d.ddddde±XX.
	e := strconv.FormatFloat(f, 'e', -1, 64)
	mant, expStr, _ := strings.Cut(e, "e")
	exp, _ := strconv.Atoi(expStr)
	digits := strings.Replace(mant, ".", "", 1)

	if exp < -4 || exp >= 16 {
		m := digits[:1]
		if len(digits) > 1 {
			m += "." + digits[1:]
		}
		es := "+"
		if exp < 0 {
			es = "-"
			exp = -exp
		}
		return fmt.Sprintf("%s%se%s%02d", sign, m, es, exp)
	}
	// Positional: decimal point after exp+1 digits.
	point := exp + 1
	var s string
	switch {
	case point <= 0:
		s = "0." + strings.Repeat("0", -point) + digits
	case point >= len(digits):
		s = digits + strings.Repeat("0", point-len(digits)) + ".0"
	default:
		s = digits[:point] + "." + digits[point:]
	}
	return sign + s
}
