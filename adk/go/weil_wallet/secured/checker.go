package secured

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"strings"
	"sync"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// VerdictTopic is the WeilStream topic secured checks publish their verdicts
// to, as in the Python SDK.
const VerdictTopic = "security"

// Option configures a Checker or Middleware.
type Option func(*options)

type options struct {
	clientOpts []client.Option
	sentinel   *client.Sentinel
	cacheTTL   time.Duration
	strict     bool
	verdicts   bool
	publisher  *client.WeilClient
	checker    *Checker
	maxAge     time.Duration
}

func newOptions(opts []Option) *options {
	o := &options{}
	for _, opt := range opts {
		opt(o)
	}
	return o
}

// WithSentinel configures how the Checker reaches Sentinel
// (client.WithSentinelHost, client.WithInsecureSkipVerify, ...).
func WithSentinel(opts ...client.Option) Option {
	return func(o *options) { o.clientOpts = append(o.clientOpts, opts...) }
}

// UsingSentinel makes the Checker use an existing Sentinel client, e.g. an
// identity's (WeilClient.Sentinel()).
func UsingSentinel(s *client.Sentinel) Option {
	return func(o *options) { o.sentinel = s }
}

// WithCacheTTL caches an org's identity applet address for d. By default it
// is resolved on every check, as the Python SDK does. Permissions themselves are never cached, so a revoked
// key is denied on the next call.
func WithCacheTTL(d time.Duration) Option {
	return func(o *options) { o.cacheTTL = d }
}

// StrictSubgroups requires the subgroup key itself for a subgroup scope. By
// default an org-level key also passes, as in the Python SDK and the on-chain
// #[secured] macro.
func StrictSubgroups() Option {
	return func(o *options) { o.strict = true }
}

// PublishVerdictsAs publishes each verdict to VerdictTopic on the scope's
// org's publisher applet ("publisher::<org>", as the Python SDK does), signed
// by publisher (typically the server's own identity), in the background.
//
// Without it no verdicts are published. The Python SDK publishes them as
// unsigned transactions from the caller's wallet; the chain accepts those
// and attributes them to the caller, so that would write on chain in
// another wallet's name. Go never does.
func PublishVerdictsAs(publisher *client.WeilClient) Option {
	return func(o *options) { o.publisher, o.verdicts = publisher, publisher != nil }
}

// WithoutVerdicts stops publishing verdicts (the default unless
// PublishVerdictsAs is given).
func WithoutVerdicts() Option {
	return func(o *options) { o.verdicts = false }
}

// Checker answers whether a wallet holds a scope's permission, by asking the
// org's identity applet (identity::<org>) key_has_purpose with unsigned
// queries, as the Python SDK's wallet_has_purpose does.
type Checker struct {
	sentinel  *client.Sentinel
	cacheTTL  time.Duration
	strict    bool
	verdicts  bool
	publisher *client.WeilClient

	mu    sync.Mutex
	cache map[string]cachedAddr
}

type cachedAddr struct {
	addr    string
	expires time.Time
}

// NewChecker returns a Checker. Options other than Sentinel, cache and
// verdict settings are ignored.
func NewChecker(opts ...Option) *Checker {
	return newChecker(newOptions(opts))
}

func newChecker(o *options) *Checker {
	s := o.sentinel
	if s == nil {
		s = client.NewSentinel(o.clientOpts...)
	}
	return &Checker{
		sentinel: s, cacheTTL: o.cacheTTL, strict: o.strict,
		verdicts: o.verdicts, publisher: o.publisher,
		cache: map[string]cachedAddr{},
	}
}

var defaultChecker = sync.OnceValue(func() *Checker { return NewChecker() })

// Check returns nil when wallet holds the permission scope and level
// demand, a *NotPermittedError when it does not, and any other error when
// the check itself could not be completed.
func (c *Checker) Check(ctx context.Context, wallet, scope string, level Level) error {
	_, err := c.check(ctx, wallet, scope, level, level.purposes())
	return err
}

// Grant is Check that also reports which purpose let wallet in, probing
// purposes in the order given (Python's _granting_purpose). With no
// purposes it probes those level User accepts. A wallet holding none gets
// "" and a *NotPermittedError.
func (c *Checker) Grant(ctx context.Context, wallet, scope string, purposes ...string) (string, error) {
	if len(purposes) == 0 {
		purposes = User.purposes()
	}
	level := Admin
	for _, p := range purposes {
		if p != PurposeManagement {
			level = User
		}
	}
	return c.check(ctx, wallet, scope, level, purposes)
}

func (c *Checker) check(ctx context.Context, wallet, scope string, level Level, purposes []string) (string, error) {
	sc, err := ParseScope(scope)
	if err != nil {
		return "", err
	}
	granted, err := c.granting(ctx, wallet, sc, purposes)
	if err != nil {
		return "", fmt.Errorf("secured: check %s for %s: %w", scope, wallet, err)
	}
	c.publishVerdict(wallet, sc, granted != "")
	if granted == "" {
		return "", &NotPermittedError{Wallet: wallet, Service: scope, Level: level}
	}
	return granted, nil
}

// Allowed is Check as a boolean; err reports only a failed check.
func (c *Checker) Allowed(ctx context.Context, wallet, scope string, level Level) (bool, error) {
	err := c.Check(ctx, wallet, scope, level)
	if errors.Is(err, ErrNotPermitted) {
		return false, nil
	}
	return err == nil, err
}

// granting returns the first purpose wallet holds in sc, or "".
func (c *Checker) granting(ctx context.Context, wallet string, sc Scope, purposes []string) (string, error) {
	if wallet == "" {
		return "", nil
	}
	// Python's _granting_purpose splits the scope at its last dot for the
	// identity applet and keys ("a.b.c" → identity::c, key "a.b_<w>"), while
	// verdicts go to the publisher of the first split; only differs for
	// scopes with more than one dot.
	sc = identityScope(sc)
	identity, err := c.identityApplet(ctx, wallet, sc)
	if err != nil {
		return "", err
	}
	// Org-level keys span every subgroup, so a subgroup scope accepts the
	// bare address as well as "<subgroup>_<addr>" (unless strict). Probed in
	// the Python SDK's order: each key in turn, every purpose for it.
	keys := []string{sc.Key(wallet)}
	if sc.Subgroup != "" {
		keys = []string{wallet, sc.Key(wallet)}
		if c.strict {
			keys = keys[1:]
		}
	}
	for _, key := range keys {
		for _, purpose := range purposes {
			has, err := c.hasPurpose(ctx, identity, wallet, key, purpose)
			if err != nil {
				return "", err
			}
			if has {
				return purpose, nil
			}
		}
	}
	return "", nil
}

// identityApplet resolves the org's identity applet, which holds the keys
// and their purposes.
// It is resolved by name alone, as the Python SDK does.
func (c *Checker) identityApplet(ctx context.Context, wallet string, sc Scope) (string, error) {
	cacheKey := sc.Org
	c.mu.Lock()
	if e, ok := c.cache[cacheKey]; ok && time.Now().Before(e.expires) {
		c.mu.Unlock()
		return e.addr, nil
	}
	c.mu.Unlock()

	id, err := c.sentinel.ResolveApplet(ctx, sc.IdentityApplet(), "")
	if err != nil {
		return "", err
	}
	if c.cacheTTL > 0 {
		c.mu.Lock()
		c.cache[cacheKey] = cachedAddr{addr: id, expires: time.Now().Add(c.cacheTTL)}
		c.mu.Unlock()
	}
	return id, nil
}

func (c *Checker) hasPurpose(ctx context.Context, applet, wallet, key, purpose string) (bool, error) {
	args, _ := client.PythonDumpsValue(struct {
		Key     string `json:"key"`
		Purpose string `json:"purpose"`
	}{key, purpose}, false)
	res, err := c.sentinel.Query(ctx, applet, "key_has_purpose", string(args), wallet)
	if err != nil {
		return false, err
	}
	v := unwrapOk(res.TxnResult)
	return v == true || v == "true", nil
}

// unwrapOk is the value inside an applet's {"Ok": ...} envelope (decoded
// once more if it is a JSON string), or nil for anything else — an Err, an
// empty or malformed result — which the checks read as "no", as the Python
// SDK's _unwrap_secured_result does.
func unwrapOk(txnResult string) any {
	var env map[string]any
	if json.Unmarshal([]byte(txnResult), &env) != nil {
		return nil
	}
	ok, found := env["Ok"]
	if !found {
		return nil
	}
	if s, isStr := ok.(string); isStr {
		var inner any
		if json.Unmarshal([]byte(s), &inner) == nil {
			return inner
		}
		return s
	}
	return ok
}

// publishVerdict records the outcome on VerdictTopic in the background; it
// never changes the outcome.
func (c *Checker) publishVerdict(wallet string, sc Scope, passed bool) {
	if !c.verdicts || c.publisher == nil || wallet == "" {
		return
	}
	verdict := "failed"
	if passed {
		verdict = "passed"
	}
	payload := fmt.Sprintf("%s %s @secured check for %s", wallet, verdict, sc)
	args, _ := client.PythonDumpsValue(struct {
		Topic   string `json:"topic"`
		Payload string `json:"payload"`
	}{VerdictTopic, payload}, false)
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), time.Minute)
		defer cancel()
		_, err := c.publisher.ExecuteByName(ctx, sc.PublisherApplet(), "publish", string(args), client.ExecuteOptions{NonBlocking: true, HideArgs: true})
		if err != nil && debugVerdicts {
			log.Printf("secured: publish verdict: %v", err)
		}
	}()
}

// debugVerdicts logs failed verdict publishes; they are best-effort.
var debugVerdicts = false

// identityScope re-splits sc at its last dot (Python's rpartition).
func identityScope(sc Scope) Scope {
	full := sc.String()
	i := strings.LastIndex(full, ".")
	if i < 0 {
		return sc
	}
	return Scope{Org: full[i+1:], Subgroup: full[:i]}
}
