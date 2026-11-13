package client

import (
	"bytes"
	"fmt"
	"io"
	"net/http"
)

// AuthTransport is an http.RoundTripper that signs every request with the
// client's wallet (see AuthHeaders): requests with a body get headers bound
// to it (AuthHeadersForBody over SignedBody), others plain ones. Headers are
// built per request, so they never go stale on long sessions.
//
// Use it for MCP clients and A2A transports talking to Weil-secured servers:
//
//	&http.Client{Transport: client.NewAuthTransport(c, nil)}
type AuthTransport struct {
	Client *WeilClient
	// Base performs the request; nil means http.DefaultTransport.
	Base http.RoundTripper
	// OnSigned, if set, is called with each signed request and the body it
	// covers (nil when unbound), e.g. to audit what was signed.
	OnSigned func(req *http.Request, signedBody any)
}

// NewAuthTransport returns an AuthTransport for c over base.
func NewAuthTransport(c *WeilClient, base http.RoundTripper) *AuthTransport {
	return &AuthTransport{Client: c, Base: base}
}

// AuthHTTPClient returns an http.Client whose requests are signed with c's
// wallet.
func (w *WeilClient) AuthHTTPClient() *http.Client {
	return &http.Client{Transport: NewAuthTransport(w, nil)}
}

func (t *AuthTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	base := t.Base
	if base == nil {
		base = http.DefaultTransport
	}
	req = req.Clone(req.Context())

	var signed any
	if req.Body != nil && req.Body != http.NoBody {
		body, err := io.ReadAll(req.Body)
		req.Body.Close()
		if err != nil {
			return nil, err
		}
		req.Body = io.NopCloser(bytes.NewReader(body))
		req.GetBody = func() (io.ReadCloser, error) { return io.NopCloser(bytes.NewReader(body)), nil }
		signed = SignedBody(body)
	}

	var (
		headers map[string]string
		err     error
	)
	if signed != nil {
		headers, err = t.Client.AuthHeadersForBody(req.Context(), signed)
	} else {
		headers, err = t.Client.AuthHeaders(req.Context())
	}
	if err != nil {
		return nil, fmt.Errorf("sign request: %w", err)
	}
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	if t.OnSigned != nil {
		t.OnSigned(req, signed)
	}
	return base.RoundTrip(req)
}
