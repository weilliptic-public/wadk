package team

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"net/http"
	"net/url"
	"regexp"
	"slices"
	"strconv"
	"strings"
	"time"
)

// HTTP is the agents' requests usage: fixed headers, an optional timeout,
// and, where the Python agents mount an HTTPAdapter, urllib3-style retries.
type HTTP struct {
	Headers map[string]string
	// Timeout per request; zero waits forever, as requests without timeout=.
	Timeout time.Duration
	// Retry, if set, retries as urllib3's Retry does.
	Retry *Retry
	// OnRequest, if set, runs before every request (the API call counters).
	OnRequest func()
}

// Retry mirrors urllib3.util.retry.Retry(total, backoff_factor,
// status_forcelist, allowed_methods).
type Retry struct {
	Total           int
	BackoffFactor   float64
	StatusForcelist []int
	AllowedMethods  []string
}

// Response is a finished HTTP exchange.
type Response struct {
	StatusCode int
	Status     string
	Header     http.Header
	Body       []byte
	URL        string
}

// HTTPError is requests.exceptions.HTTPError, with its message.
type HTTPError struct {
	Response *Response
}

func (e *HTTPError) Error() string {
	kind := "Client"
	if e.Response.StatusCode >= 500 {
		kind = "Server"
	}
	return fmt.Sprintf("%d %s Error: %s for url: %s", e.Response.StatusCode, kind,
		http.StatusText(e.Response.StatusCode), e.Response.URL)
}

// RaiseForStatus is response.raise_for_status().
func (r *Response) RaiseForStatus() error {
	if r.StatusCode >= 400 {
		return &HTTPError{Response: r}
	}
	return nil
}

// OK is response.ok.
func (r *Response) OK() bool { return r.StatusCode < 400 }

// Text is response.text.
func (r *Response) Text() string { return string(r.Body) }

// JSON is response.json(): objects become map[string]any, arrays []any,
// numbers float64.
func (r *Response) JSON() (any, error) {
	var v any
	if err := json.Unmarshal(r.Body, &v); err != nil {
		return nil, fmt.Errorf("Expecting value: %v", err)
	}
	return v, nil
}

var linkRE = regexp.MustCompile(`<([^>]*)>\s*;\s*rel="?([^";]+)"?`)

// Links is response.links: rel → url of the Link header.
func (r *Response) Links() map[string]string {
	out := map[string]string{}
	for _, m := range linkRE.FindAllStringSubmatch(r.Header.Get("Link"), -1) {
		out[m[2]] = m[1]
	}
	return out
}

// Params is a query; values are formatted with fmt.Sprint.
type Params map[string]any

// Do sends one request. body, if non-nil, is sent as JSON. extra headers
// override the client's.
func (c *HTTP) Do(method, rawURL string, params Params, body any, extra map[string]string) (*Response, error) {
	if len(params) > 0 {
		u, err := url.Parse(rawURL)
		if err != nil {
			return nil, err
		}
		q := u.Query()
		for k, v := range params {
			q.Set(k, fmt.Sprint(v))
		}
		u.RawQuery = q.Encode()
		rawURL = u.String()
	}
	var payload []byte
	if body != nil {
		b, err := json.Marshal(body)
		if err != nil {
			return nil, err
		}
		payload = b
	}
	if c.OnRequest != nil {
		c.OnRequest()
	}
	client := &http.Client{Timeout: c.Timeout}
	for attempt := 0; ; attempt++ {
		var rd io.Reader
		if payload != nil {
			rd = bytes.NewReader(payload)
		}
		req, err := http.NewRequest(method, rawURL, rd)
		if err != nil {
			return nil, err
		}
		for k, v := range c.Headers {
			req.Header.Set(k, v)
		}
		for k, v := range extra {
			req.Header.Set(k, v)
		}
		if payload != nil {
			req.Header.Set("Content-Type", "application/json")
		}
		resp, err := client.Do(req)
		retryable := c.Retry != nil && slices.Contains(c.Retry.AllowedMethods, method)
		if err != nil {
			if retryable && attempt < c.Retry.Total {
				time.Sleep(c.Retry.backoff(attempt+1, nil))
				continue
			}
			return nil, err
		}
		data, err := io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			return nil, err
		}
		out := &Response{StatusCode: resp.StatusCode, Status: resp.Status, Header: resp.Header, Body: data, URL: rawURL}
		if retryable && slices.Contains(c.Retry.StatusForcelist, resp.StatusCode) {
			if attempt < c.Retry.Total {
				time.Sleep(c.Retry.backoff(attempt+1, resp.Header))
				continue
			}
			u, _ := url.Parse(rawURL)
			return nil, fmt.Errorf("HTTPSConnectionPool(host='%s', port=443): Max retries exceeded with url: %s (Caused by ResponseError('too many %d error responses'))",
				u.Host, u.RequestURI(), resp.StatusCode)
		}
		return out, nil
	}
}

// backoff is urllib3's: Retry-After when the server sends it, else none
// before the first retry and factor·2^(n-1) after, capped at 120s.
func (r *Retry) backoff(n int, h http.Header) time.Duration {
	if h != nil {
		if s := h.Get("Retry-After"); s != "" {
			if secs, err := strconv.Atoi(strings.TrimSpace(s)); err == nil {
				return time.Duration(secs) * time.Second
			}
			if t, err := http.ParseTime(s); err == nil {
				return max(time.Until(t), 0)
			}
		}
	}
	if n <= 1 {
		return 0
	}
	secs := math.Min(r.BackoffFactor*math.Pow(2, float64(n-1)), 120)
	return time.Duration(secs * float64(time.Second))
}

// GetJSON is requests.get(...); raise_for_status(); .json().
func (c *HTTP) GetJSON(rawURL string, params Params) (any, error) {
	return c.sendJSON(http.MethodGet, rawURL, params, nil)
}

// PostJSON is requests.post(json=payload); raise_for_status(); .json().
func (c *HTTP) PostJSON(rawURL string, payload any) (any, error) {
	return c.sendJSON(http.MethodPost, rawURL, nil, payload)
}

// PatchJSON is requests.patch(json=payload); raise_for_status(); .json().
func (c *HTTP) PatchJSON(rawURL string, payload any) (any, error) {
	return c.sendJSON(http.MethodPatch, rawURL, nil, payload)
}

func (c *HTTP) sendJSON(method, rawURL string, params Params, payload any) (any, error) {
	resp, err := c.Do(method, rawURL, params, payload, nil)
	if err != nil {
		return nil, err
	}
	if err := resp.RaiseForStatus(); err != nil {
		return nil, err
	}
	return resp.JSON()
}

// GraphQL posts {"query", "variables"} to url and returns the decoded
// response (run_graphql_query).
func (c *HTTP) GraphQL(rawURL, query string, variables map[string]any) (map[string]any, error) {
	v, err := c.PostJSON(rawURL, map[string]any{"query": query, "variables": variables})
	if err != nil {
		return nil, err
	}
	m, _ := v.(map[string]any)
	if m == nil {
		m = map[string]any{}
	}
	return m, nil
}

// GitHubHeaders are the headers most of the agents send.
func GitHubHeaders(token string) map[string]string {
	return map[string]string{
		"Authorization": "token " + token,
		"Accept":        "application/vnd.github.v3+json",
	}
}
