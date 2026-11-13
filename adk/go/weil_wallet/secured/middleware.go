package secured

import (
	"bytes"
	"context"
	"io"
	"net/http"
	"sync"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

type callerKey struct{}
type checkerKey struct{}

// ContextWithCaller returns ctx carrying a verified caller, as Middleware
// does. Use it to serve callers verified some other way.
func ContextWithCaller(ctx context.Context, caller *client.AuthCaller) context.Context {
	return context.WithValue(ctx, callerKey{}, caller)
}

// CallerFromContext returns the verified caller of the request being served.
func CallerFromContext(ctx context.Context) (*client.AuthCaller, bool) {
	if ctx == nil {
		return nil, false
	}
	c, ok := ctx.Value(callerKey{}).(*client.AuthCaller)
	return c, ok && c != nil
}

// WalletFromContext returns the verified caller's account address, or ""
// outside a verified request (Python: current_wallet_addr).
func WalletFromContext(ctx context.Context) string {
	if c, ok := CallerFromContext(ctx); ok {
		return c.Address
	}
	return ""
}

// ContextWithChecker makes gates under ctx use c.
func ContextWithChecker(ctx context.Context, c *Checker) context.Context {
	return context.WithValue(ctx, checkerKey{}, c)
}

// CheckerFromContext returns the Checker set by ContextWithChecker, or nil.
func CheckerFromContext(ctx context.Context) *Checker {
	if ctx == nil {
		return nil
	}
	c, _ := ctx.Value(checkerKey{}).(*Checker)
	return c
}

func checkerFrom(ctx context.Context) *Checker {
	if c := CheckerFromContext(ctx); c != nil {
		return c
	}
	return defaultChecker()
}

// WithChecker makes Middleware hand c to the gates it serves, instead of
// one built from the other options.
func WithChecker(c *Checker) Option {
	return func(o *options) { o.checker = c }
}

// WithMaxAge bounds the age of auth headers (default client.MaxAuthAge).
func WithMaxAge(d time.Duration) Option {
	return func(o *options) { o.maxAge = d }
}

// Middleware verifies the Weil auth headers of every POST request — the
// Go twin of weil_ai's weil_middleware — and serves next with the verified
// caller (CallerFromContext) and the Checker gates use. Requests that fail
// get 401. Other methods pass through unverified, as an MCP GET only opens
// the event stream and never runs a tool.
//
// Headers bound to the body (X-Body-SHA256) must match it, and their nonce
// is accepted once.
func Middleware(next http.Handler, opts ...Option) http.Handler {
	o := newOptions(opts)
	checker := o.checker
	if checker == nil {
		checker = newChecker(o)
	}
	replay := newNonceCache(o.maxAge)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			next.ServeHTTP(w, r)
			return
		}
		caller, err := VerifyRequest(r, o.maxAge)
		if err == nil && caller.Nonce != "" && !replay.add(caller.Nonce) {
			err = &client.AuthError{Reason: "nonce_reused"}
		}
		if err != nil {
			// Exactly the Python middleware's response body.
			// No Content-Type, as Starlette's Response; nil also stops Go
			// from sniffing one.
			w.Header()["Content-Type"] = nil
			w.WriteHeader(http.StatusUnauthorized)
			_, _ = w.Write([]byte("Wallet address verification failed"))
			return
		}
		ctx := ContextWithChecker(ContextWithCaller(r.Context(), caller), checker)
		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// VerifyRequest verifies r's auth headers, and its body when they are bound
// to one; the body stays readable for the next handler.
func VerifyRequest(r *http.Request, maxAge time.Duration) (*client.AuthCaller, error) {
	if r.Header.Get(client.HeaderBodySHA256) == "" {
		return client.VerifyAuthHeaders(r.Header, maxAge)
	}
	body, err := io.ReadAll(r.Body)
	if err != nil {
		return nil, err
	}
	r.Body.Close()
	r.Body = io.NopCloser(bytes.NewReader(body))
	return client.VerifyAuthRequest(r.Header, client.SignedBody(body), maxAge)
}

// nonceCache remembers nonces for as long as their headers stay fresh.
type nonceCache struct {
	ttl  time.Duration
	mu   sync.Mutex
	seen map[string]time.Time
}

func newNonceCache(maxAge time.Duration) *nonceCache {
	if maxAge <= 0 {
		maxAge = client.MaxAuthAge
	}
	return &nonceCache{ttl: 2 * maxAge, seen: map[string]time.Time{}}
}

// add records nonce and reports whether it was new.
func (n *nonceCache) add(nonce string) bool {
	n.mu.Lock()
	defer n.mu.Unlock()
	now := time.Now()
	if len(n.seen) > 1024 {
		for k, at := range n.seen {
			if now.Sub(at) > n.ttl {
				delete(n.seen, k)
			}
		}
	}
	if at, ok := n.seen[nonce]; ok && now.Sub(at) <= n.ttl {
		return false
	}
	n.seen[nonce] = now
	return true
}
