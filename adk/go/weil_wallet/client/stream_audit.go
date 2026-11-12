package client

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"mime"
	"net/http"
	"net/url"
	"strings"
	"unicode/utf8"
)

// ErrNoOrg is returned by StreamAudit when the active account is linked to
// no organization, so there is no publisher::<org> to publish to.
var ErrNoOrg = errors.New("stream_audit requires a wallet linked to an organization")

// StreamAudit returns HTTP middleware that publishes each request's payload
// to topic on the active account's org publisher ("publisher::<org>") before
// the handler runs — the Python SDK's @client.stream_audit(topic) on a
// FastAPI handler. A failed publish fails the request (500), as the
// decorator's exception does.
//
// The payload is the request body as Python extracts it: a JSON body
// re-encoded as json.dumps does, a form as an object of its fields, other
// bodies as UTF-8 text (re-decoded if it is JSON) or else hex, and {} when
// empty.
func (w *WeilClient) StreamAudit(topic string) func(http.Handler) http.Handler {
	if topic == "" {
		panic("stream_audit topic must be non-empty")
	}
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(rw http.ResponseWriter, r *http.Request) {
			body, err := io.ReadAll(r.Body)
			r.Body.Close()
			if err != nil {
				http.Error(rw, "could not read request", http.StatusBadRequest)
				return
			}
			r.Body = io.NopCloser(bytes.NewReader(body))
			payload, err := streamAuditPayload(r.Header.Get("Content-Type"), body)
			if err == nil {
				err = w.StreamAuditPayload(r.Context(), topic, payload)
			}
			if err != nil {
				http.Error(rw, "Internal Server Error", http.StatusInternalServerError)
				return
			}
			r.Body = io.NopCloser(bytes.NewReader(body))
			next.ServeHTTP(rw, r)
		})
	}
}

// StreamAuditPayload publishes payload (already encoded) to topic on the
// active account's org publisher, waiting for it to land, as stream_audit
// does for a wrapped call.
func (w *WeilClient) StreamAuditPayload(ctx context.Context, topic, payload string) error {
	org := w.Org()
	if org == nil || org.Name == "" {
		return ErrNoOrg
	}
	_, err := w.PublishTo(ctx, "publisher::"+org.Name, topic, payload, ExecuteOptions{})
	return err
}

func streamAuditPayload(contentType string, body []byte) (string, error) {
	mediaType, _, _ := mime.ParseMediaType(contentType)
	mediaType = strings.ToLower(mediaType)
	switch {
	case mediaType == "application/json" || strings.HasSuffix(mediaType, "+json"):
		if out, err := PythonDumps(body, false); err == nil {
			return string(out), nil
		}
	case mediaType == "application/x-www-form-urlencoded":
		// Fields in first-seen order, each with its last value, as Python's
		// {k: v for k, v in form.multi_items()}.
		var keys []string
		values := map[string]string{}
		for _, pair := range strings.Split(string(body), "&") {
			if pair == "" {
				continue
			}
			k, v, _ := strings.Cut(pair, "=")
			k, err := url.QueryUnescape(k)
			if err != nil {
				return "", err
			}
			v, err = url.QueryUnescape(v)
			if err != nil {
				return "", err
			}
			if _, seen := values[k]; !seen {
				keys = append(keys, k)
			}
			values[k] = v
		}
		var raw bytes.Buffer
		raw.WriteByte('{')
		for i, k := range keys {
			if i > 0 {
				raw.WriteByte(',')
			}
			kb, _ := json.Marshal(k)
			vb, _ := json.Marshal(values[k])
			raw.Write(kb)
			raw.WriteByte(':')
			raw.Write(vb)
		}
		raw.WriteByte('}')
		out, err := PythonDumps(raw.Bytes(), false)
		return string(out), err
	}
	var v any = map[string]any{}
	if len(body) > 0 {
		if !utf8.Valid(body) {
			v = hex.EncodeToString(body)
		} else if json.Valid(body) {
			out, err := PythonDumps(body, false)
			return string(out), err
		} else {
			v = string(body)
		}
	}
	out, err := PythonDumpsValue(v, false)
	return string(out), err
}
