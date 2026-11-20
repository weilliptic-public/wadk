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

// StrictSubgroups requires the subgroup key itself for a subgroup condition,
// and skips the org-level shortcut. By default an org-level key also passes,
// as in the Python SDK and the on-chain #[secured] macro.
func StrictSubgroups() Option {
	return func(o *options) { o.strict = true }
}

// PublishVerdictsAs publishes each verdict to VerdictTopic on the publisher
// applet of every org the rule names ("publisher::<org>"), signed by
// publisher (typically the server's own identity), in the background.
//
// Without it no verdicts are published.
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

// Check returns nil when wallet satisfies the secured rule (one scope like
// "engg.weil", or a Rule over several) at level, a *NotPermittedError when
// it does not, and any other error — a *CheckError when a check the rule
// needed failed — when the outcome could not be decided.
func (c *Checker) Check(ctx context.Context, wallet, scope string, level Level) error {
	_, err := c.check(ctx, wallet, scope, level, level.purposes())
	return err
}

// Grant is Check that also reports which purpose let wallet in, probing
// purposes in the order given, and stopping at the first held. With no
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

func (c *Checker) check(ctx context.Context, wallet, rule string, level Level, purposes []string) (string, error) {
	r, err := parseGateRule(rule)
	if err != nil {
		return "", err
	}
	granted, outcome, failures := c.granting(ctx, wallet, r, purposes)
	// One verdict per org the rule names, to that org's publisher; an
	// undecided rule is a failed check, as in the Python SDK.
	c.publishVerdicts(wallet, rule, Orgs(r), outcome == True)
	switch outcome {
	case True:
		return granted, nil
	case Unknown:
		return "", fmt.Errorf("secured: check %s for %s: %w", rule, wallet, &CheckError{Rule: rule, Failures: failures})
	}
	return "", &NotPermittedError{Wallet: wallet, Service: rule, Level: level}
}

// Allowed is Check as a boolean; err reports only a failed check.
func (c *Checker) Allowed(ctx context.Context, wallet, scope string, level Level) (bool, error) {
	err := c.Check(ctx, wallet, scope, level)
	if errors.Is(err, ErrNotPermitted) {
		return false, nil
	}
	return err == nil, err
}

// CheckError is returned (wrapped) when a rule's outcome could not be
// decided: a check it depended on failed (identity applet missing, a
// key_has_purpose call errored) and the rest of the rule could not decide
// without it. Gates treat it as not permitted.
// A failure on a branch the rule did not need does not cause one.
type CheckError struct {
	Rule     string
	Failures []string
}

func (e *CheckError) Error() string {
	detail := strings.Join(e.Failures, "; ")
	if detail == "" {
		detail = "unknown failure"
	}
	return fmt.Sprintf("secured check for %q could not be completed: %s", e.Rule, detail)
}

// granting evaluates r for wallet root-first (EvaluateRootFirst), each key
// probed for purposes in the order given. granted is the first purpose
// that matched while evaluating — or, for a rule satisfied with no key
// matching (only ! conditions), the first one listed.
func (c *Checker) granting(ctx context.Context, wallet string, r Rule, purposes []string) (granted string, outcome Outcome, failures []string) {
	if wallet == "" {
		return "", False, nil
	}
	type applet struct {
		addr string
		ok   bool
	}
	applets := map[string]applet{}
	var matched []string
	appletFor := func(ctx context.Context, org string) (string, bool) {
		if a, seen := applets[org]; seen {
			return a.addr, a.ok
		}
		addr, err := c.identityApplet(ctx, org)
		if err != nil {
			failures = append(failures, fmt.Sprintf("%s did not resolve: %v", identityApplet(org), err))
		}
		applets[org] = applet{addr, err == nil}
		return addr, err == nil
	}
	checkKey := func(ctx context.Context, org, key string) Outcome {
		addr, ok := appletFor(ctx, org)
		if !ok {
			return Unknown
		}
		unknown := false
		for _, purpose := range purposes {
			has, err := c.hasPurpose(ctx, addr, wallet, key, purpose)
			if err != nil {
				failures = append(failures, fmt.Sprintf("key_has_purpose(%q, %s) on %q failed: %v", key, purpose, org, err))
				unknown = true
				continue
			}
			if has {
				matched = append(matched, purpose)
				return True
			}
		}
		if unknown {
			return Unknown
		}
		return False
	}
	outcome = evaluateRootFirst(ctx, r, wallet, c.strict, checkKey)
	if outcome == True {
		switch {
		case len(matched) > 0:
			granted = matched[0]
		case len(purposes) > 0:
			granted = purposes[0]
		}
	}
	return granted, outcome, failures
}

// identityApplet resolves the org's identity applet, which holds the keys
// and their purposes.
// It is resolved by name alone, as the Python SDK does.
func (c *Checker) identityApplet(ctx context.Context, org string) (string, error) {
	cacheKey := org
	c.mu.Lock()
	if e, ok := c.cache[cacheKey]; ok && time.Now().Before(e.expires) {
		c.mu.Unlock()
		return e.addr, nil
	}
	c.mu.Unlock()

	id, err := c.sentinel.ResolveApplet(ctx, identityApplet(org), "")
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

// publishVerdicts records the outcome on VerdictTopic of each org's
// publisher applet in the background; it never changes the outcome.
func (c *Checker) publishVerdicts(wallet, rule string, orgs []string, passed bool) {
	if !c.verdicts || c.publisher == nil || wallet == "" {
		return
	}
	verdict := "failed"
	if passed {
		verdict = "passed"
	}
	payload := fmt.Sprintf("%s %s @secured check for %s", wallet, verdict, rule)
	args, _ := client.PythonDumpsValue(struct {
		Topic   string `json:"topic"`
		Payload string `json:"payload"`
	}{VerdictTopic, payload}, false)
	for _, org := range orgs {
		go func() {
			ctx, cancel := context.WithTimeout(context.Background(), time.Minute)
			defer cancel()
			_, err := c.publisher.ExecuteByName(ctx, publisherApplet(org), "publish", string(args), client.ExecuteOptions{NonBlocking: true, HideArgs: true})
			if err != nil && debugVerdicts {
				log.Printf("secured: publish verdict: %v", err)
			}
		}()
	}
}

// debugVerdicts logs failed verdict publishes; they are best-effort.
var debugVerdicts = false
