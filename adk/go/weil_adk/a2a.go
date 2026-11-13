package weiladk

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"slices"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/a2aproject/a2a-go/v2/a2a"
	"github.com/a2aproject/a2a-go/v2/a2aclient"
	"github.com/a2aproject/a2a-go/v2/a2asrv"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/remoteagent/v2"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/server/adka2a/v2"
	"google.golang.org/adk/v2/session"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

// Secure agent-to-agent (A2A) calls, wire-compatible with the Python SDK
// (weil_ai.adk), so Go and Python agents call each other:
//
//	Python (weil_ai)                          Go (weiladk)
//	agent.setup_secure_a2a()  (caller)        weiladk.NewA2A(id, remoteagent.A2AConfig{...})
//	agent.to_secure_a2a(service=...)          weiladk.SecureA2A(handler, agent, weiladk.WithService(...))
//	                                          + weiladk.NewExecutor(adka2a.ExecutorConfig{...})
//	@secured_user / @secured_admin on tools   weiladk.SecuredUser / weiladk.SecuredAdmin
//	caller_wallet in audit records            weiladk.CallerFromContext(ctx)
//
// The caller signs every HTTP request with its wallet, bound to the A2A
// message it carries (X-Wallet-Address, X-Signature, X-Message,
// X-Timestamp, X-Body-SHA256, X-Nonce). The server verifies the signature,
// the body and the nonce, checks the caller's permission on the service
// scope on chain, and serves the agent with the verified caller in context.
// Both sides write the same audit events to the auditor applet.
//
// The caller is authorized once, at that boundary. Inside the agent tree,
// @secured tools authorize as the agent running them (see SecuredUser), and
// audit records are signed by that agent, with caller_wallet naming who
// invoked it.

// Caller is who invoked the acting agent, available to the agent's hooks
// (Event.Caller) and tools (CallerFromContext).
type Caller = client.AuthCaller

// CallerFromContext returns who invoked the agent acting in ctx — what the
// Python SDK records as caller_wallet:
//
//   - for an agent with an identity of its own (Bind) nested under another
//     bound agent, that enclosing agent's wallet;
//   - otherwise, in a run serving an A2A request (SecureA2A), the request's
//     verified caller;
//   - otherwise (a local run of the root agent) none.
//
// It is not who @secured tools authorize as: they run as the acting agent
// (IdentityOf).
func CallerFromContext(ctx context.Context) (*Caller, bool) {
	if ctx == nil {
		return nil, false
	}
	if a := actingAgent(ctx); a != nil && boundTo(a) != nil {
		if parent := enclosingOf(a); parent != nil {
			return &Caller{Address: parent.Address(), Canonical: true}, true
		}
	}
	return secured.CallerFromContext(ctx)
}

// servedKey marks a context serving an A2A request through SecureA2A.
type servedKey struct{}

// ── Options ───────────────────────────────────────────────────────────────

// A2AOption configures NewA2A, SecureA2A and NewExecutor; each ignores the
// options that do not apply to it.
type A2AOption func(*a2aOptions)

type a2aOptions struct {
	identity       *Identity
	service        string
	authorize      func(ctx context.Context, caller *Caller) error
	skipVerify     bool
	maxAge         time.Duration
	checker        *secured.Checker
	noAudit        bool
	noAgentAudit   bool
	dispatcher     *Dispatcher
	expectedWallet string
}

func newA2AOptions(opts []A2AOption) *a2aOptions {
	o := &a2aOptions{}
	for _, opt := range opts {
		opt(o)
	}
	if o.dispatcher == nil {
		o.dispatcher = DefaultDispatcher()
	}
	return o
}

// WithService gates a served agent on its callers holding the Execution or
// Management purpose in scope ("subgroup.org" or "org"), as
// to_secure_a2a(service=...) does. Without it any verified caller is served.
func WithService(scope string) A2AOption {
	return func(o *a2aOptions) { o.service = scope }
}

// WithAuthorize adds your own decision after the caller is verified and has
// passed the service gate, e.g. an allowlist; an error refuses the request
// with 403.
func WithAuthorize(f func(ctx context.Context, caller *Caller) error) A2AOption {
	return func(o *a2aOptions) { o.authorize = f }
}

// AllowAnonymousCallers no longer has any effect: as in the Python SDK, a
// request without a caller identity is always refused, so no run starts
// without a named caller.
//
// Deprecated: anonymous requests are refused; sign them (NewA2A,
// Identity.HTTPClient).
func AllowAnonymousCallers() A2AOption {
	return func(*a2aOptions) {}
}

// WithoutSignatureVerification serves requests on their X-Wallet-Address
// header without checking the signature — Python's
// setup_secure_a2a(verify_signature=False). A request with no wallet is still
// refused, the service gate still applies, and a2a.request.accepted records
// "verified": false. Only for trusted networks.
func WithoutSignatureVerification() A2AOption {
	return func(o *a2aOptions) { o.skipVerify = true }
}

// WithMaxAge bounds the age of request signatures (default 5 minutes).
func WithMaxAge(d time.Duration) A2AOption {
	return func(o *a2aOptions) { o.maxAge = d }
}

// WithChecker sets the permission checker. By default it asks the served
// identity's Sentinel and publishes verdicts signed by that identity.
func WithChecker(c *secured.Checker) A2AOption {
	return func(o *a2aOptions) { o.checker = c }
}

// WithServerIdentity sets the served agent's identity explicitly, when the
// agent is not bound (Bind / NewLLMAgent) or the executor uses a
// RunnerProvider.
func WithServerIdentity(id *Identity) A2AOption {
	return func(o *a2aOptions) { o.identity = id }
}

// WithoutAudit stops writing A2A audit events to the auditor applet.
func WithoutAudit() A2AOption {
	return func(o *a2aOptions) { o.noAudit = true }
}

// WithoutAgentAudit stops only the adk.agent.* and adk.tool.* records of a
// served agent (NewExecutor, ToSecureA2A), keeping the a2a.* records of the
// gate — Python's setup_secure_a2a(audit=False) behind to_secure_a2a.
func WithoutAgentAudit() A2AOption {
	return func(o *a2aOptions) { o.noAgentAudit = true }
}

// WithAuditDispatcher runs the executor plugin's queued work (run traces) on
// d instead of the default dispatcher. Audit records are not queued: they are
// submitted inline, in order, as the Python SDK awaits them.
func WithAuditDispatcher(d *Dispatcher) A2AOption {
	return func(o *a2aOptions) { o.dispatcher = d }
}

// WithExpectedWallet pins the remote agent's wallet on NewA2A: calls fail
// unless the remote card proves this wallet (see SignAgentCard).
func WithExpectedWallet(wallet string) A2AOption {
	return func(o *a2aOptions) { o.expectedWallet = wallet }
}

// ── Client: weiladk.NewA2A, the twin of remoteagent.NewA2A ────────────────

// NewA2A is remoteagent.NewA2A with on-chain identity: the same A2AConfig,
// plus the identity that signs every request sent to the remote agent —
// what setup_secure_a2a() does for a Python caller. The returned agent is
// bound to id.
//
// Requests go through a signing HTTP client, so leave
// A2AConfig.ClientProvider unset (or build it with A2AClientFactory).
func NewA2A(id *Identity, cfg remoteagent.A2AConfig, opts ...A2AOption) (agent.Agent, error) {
	if id == nil {
		return nil, ErrNoIdentity
	}
	o := newA2AOptions(opts)

	callee := &atomic.Pointer[CardIdentity]{}
	target := &atomic.Pointer[string]{}
	check := func(card *a2a.AgentCard) error {
		if card != nil && len(card.SupportedInterfaces) > 0 && card.SupportedInterfaces[0] != nil {
			u := card.SupportedInterfaces[0].URL
			target.Store(&u)
		}
		ci, err := VerifyAgentCard(card)
		switch {
		case errors.Is(err, ErrNoCardIdentity) && !hasIdentityExtension(card) && o.expectedWallet == "":
			return nil // a plain card, as Python agents serve
		case err != nil:
			return err
		case o.expectedWallet != "" && ci.Wallet != o.expectedWallet:
			return fmt.Errorf("weiladk: remote agent is wallet %s, expected %s", ci.Wallet, o.expectedWallet)
		}
		callee.Store(ci)
		return nil
	}
	if cfg.AgentCard != nil {
		if err := check(cfg.AgentCard); err != nil {
			return nil, err
		}
	}
	if provide := cfg.AgentCardProvider; provide != nil {
		cfg.AgentCardProvider = func(ctx context.Context) (*a2a.AgentCard, error) {
			card, err := provide(ctx)
			if err != nil {
				return nil, err
			}
			return card, check(card)
		}
	}
	if cfg.ClientProvider != nil {
		return nil, errors.New("weiladk: NewA2A signs requests through its own ClientProvider; leave A2AConfig.ClientProvider unset")
	}

	// Python's audits name the remote agent only: its target_url always
	// comes out empty (the ADK config it reads has no url), so it is left
	// out here too.
	targetFields := func() map[string]any { return map[string]any{"target": cfg.Name} }

	transport := &client.AuthTransport{Client: id.client}
	if !o.noAudit {
		transport.OnSigned = func(req *http.Request, signed any) {
			if req.Method != http.MethodPost {
				return
			}
			msg := messageFields(signed)
			// Python audits from the send-message interceptor: requests
			// that carry no A2A message (tasks/get, cancel...) are not
			// audited.
			if msg["message_id"] == nil {
				return
			}
			fields := merge(map[string]any{
				"direction":          "outbound",
				"wallet":             id.Address(),
				"signed_body_sha256": shortHash(req.Header.Get(client.HeaderBodySHA256)),
			}, targetFields(), msg)
			o.audit(id, "a2a.request.signed", fields)
		}
		cfg.AfterRequestCallbacks = append(slices.Clone(cfg.AfterRequestCallbacks),
			func(ctx agent.Context, req *a2a.SendMessageRequest, resp *session.Event, err error) (*session.Event, error) {
				if resp != nil && resp.Partial {
					return nil, nil
				}
				o.audit(id, "a2a.response.received", merge(map[string]any{
					"direction": "inbound",
					"wallet":    id.Address(),
				}, targetFields(), responseFields(resp, err)))
				return nil, nil
			})
	}
	cfg.ClientProvider = remoteagent.NewA2AClientProvider(A2AClientFactory(&http.Client{Transport: transport}))

	a, err := remoteagent.NewA2A(cfg)
	if err != nil {
		return nil, err
	}
	remoteIdentities.Store(a, callee)
	return Bind(a, id), nil
}

// A2AClientFactory returns an a2aclient.Factory whose JSON-RPC and REST
// transports use httpClient — e.g. one from Identity.HTTPClient, for A2A
// clients built by hand.
func A2AClientFactory(httpClient *http.Client) *a2aclient.Factory {
	return a2aclient.NewFactory(
		a2aclient.WithJSONRPCTransport(httpClient),
		a2aclient.WithRESTTransport(httpClient),
		a2aclient.WithCallInterceptors(emptyPartFilter{}),
	)
}

// emptyPartFilter drops reply parts that carry no content, such as the
// empty thought part a Python ADK agent sends for a thinking block with
// nothing to show ({"metadata": {"adk_thought": true}}). Such a part is
// valid A2A, but ADK's conversion of the reply fails on it ("unknown part
// type"), which would fail the whole call to the remote agent.
type emptyPartFilter struct {
	a2aclient.PassthroughInterceptor
}

func (emptyPartFilter) After(_ context.Context, resp *a2aclient.Response) error {
	switch v := resp.Payload.(type) {
	case *a2a.Message:
		stripMessage(v)
	case *a2a.Task:
		for _, m := range v.History {
			stripMessage(m)
		}
		for _, a := range v.Artifacts {
			stripArtifact(a)
		}
		stripMessage(v.Status.Message)
	case *a2a.TaskStatusUpdateEvent:
		stripMessage(v.Status.Message)
	case *a2a.TaskArtifactUpdateEvent:
		stripArtifact(v.Artifact)
	}
	return nil
}

func stripMessage(m *a2a.Message) {
	if m != nil {
		m.Parts = withContent(m.Parts)
	}
}

func stripArtifact(a *a2a.Artifact) {
	if a != nil {
		a.Parts = withContent(a.Parts)
	}
}

func withContent(parts []*a2a.Part) []*a2a.Part {
	out := make([]*a2a.Part, 0, len(parts))
	for _, p := range parts {
		if p != nil && (p.Text() != "" || p.Data() != nil || p.Raw() != nil || p.URL() != "") {
			out = append(out, p)
		}
	}
	return out
}

// remoteIdentities records the card identity each remote agent proved.
var remoteIdentities sync.Map // agent.Agent → *atomic.Pointer[CardIdentity]

// RemoteIdentity returns the identity a remote agent's card proved, if its
// card is signed (see SignAgentCard) and has been resolved.
func RemoteIdentity(a agent.Agent) (*CardIdentity, bool) {
	v, ok := remoteIdentities.Load(a)
	if !ok {
		return nil, false
	}
	ci := v.(*atomic.Pointer[CardIdentity]).Load()
	return ci, ci != nil
}

// writePlain writes body as the whole response, as the Python middleware's
// _send_plain does (http.Error would add a newline and extra headers).
func writePlain(w http.ResponseWriter, status int, body string) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.WriteHeader(status)
	_, _ = io.WriteString(w, body)
}

// unverifiedCaller is the caller named by headers that were not verified
// (WithoutSignatureVerification): the signed canonical address when its
// suffix matches X-Wallet-Address, else the header itself, as Python's
// full_address_from_headers falls back.
func unverifiedCaller(h http.Header) *Caller {
	header := h.Get(client.HeaderWalletAddress)
	c := &Caller{Address: header, KeyAddress: header}
	var signed struct {
		Address string `json:"address"`
	}
	if json.Unmarshal([]byte(h.Get(client.HeaderMessage)), &signed) == nil && signed.Address != "" &&
		strings.HasSuffix(strings.ToLower(signed.Address), strings.ToLower(header)) {
		c.Address, c.Canonical = signed.Address, true
	}
	return c
}

// verifyFailure says why verification failed, in the Python SDK's order
// (_verify_failure_context): missing headers, malformed or stale timestamp,
// malformed signed message, body hash mismatch, unbound body, then signature
// mismatch. Go's replay checks (nonce_reused, timestamp_mismatch), which
// Python lacks, are reported as such.
func verifyFailure(h http.Header, signed any, err error) map[string]any {
	var ae *client.AuthError
	if errors.As(err, &ae) {
		switch ae.Reason {
		case "missing_auth_headers", "malformed_timestamp", "stale_timestamp", "nonce_reused", "timestamp_mismatch":
			f := map[string]any{"reason": ae.Reason}
			for k, v := range ae.Detail {
				f[k] = v
			}
			return f
		}
	}
	var msg map[string]any
	if json.Unmarshal([]byte(h.Get(client.HeaderMessage)), &msg) != nil {
		return map[string]any{"reason": "malformed_signed_message"}
	}
	signedHash, _ := msg["body_sha256"].(string)
	actual, _ := client.BodySHA256(signed)
	if signedHash != "" && actual != "" && signedHash != actual {
		return map[string]any{"reason": "body_hash_mismatch",
			"signed_body_sha256": shortHash(signedHash), "actual_body_sha256": shortHash(actual)}
	}
	if signedHash == "" {
		return map[string]any{"reason": "unbound_body", "detail": "signed message omits body_sha256"}
	}
	return map[string]any{"reason": "signature_mismatch"}
}

// rootCause is the innermost error err wraps.
func rootCause(err error) error {
	for {
		next := errors.Unwrap(err)
		if next == nil {
			return err
		}
		err = next
	}
}

// errorType names an error's type for audits, as Python's
// type(exc).__name__ does: "AppletError" for *transaction.AppletError.
func errorType(err error) string {
	t := fmt.Sprintf("%T", err)
	if i := strings.LastIndex(t, "."); i >= 0 {
		t = t[i+1:]
	}
	return strings.TrimLeft(t, "*")
}

// auditKeyOrder is the order the Python SDK writes each audit record's
// fields in (its _audit keyword order, after "event" and "ts"); fields it
// does not write come last, sorted.
var auditKeyOrder = func() map[string][]string {
	request := []string{"path", "peer", "user_agent", "body_bytes", "body_sha256", "rpc_method", "rpc_id", "message_id", "task_id", "context_id"}
	serving := []string{"agent", "agent_wallet"}
	caller := []string{"wallet", "wallet_header", "wallet_source"}
	check := []string{"service", "permission_check", "required_purposes", "sentinel_host"}
	verify := []string{"reason", "missing_headers", "timestamp", "timestamp_skew_s", "max_age_s", "signed_body_sha256", "actual_body_sha256", "detail"}
	tool := []string{"agent", "tool", "tool_call_id", "service", "access", "wallet", "caller_wallet"}
	agentRun := []string{"agent", "agent_type", "service", "wallet", "caller_wallet"}
	cat := func(parts ...[]string) []string {
		var out []string
		for _, p := range parts {
			out = append(out, p...)
		}
		return out
	}
	return map[string][]string{
		"adk.agent.before":      agentRun,
		"adk.agent.after":       agentRun,
		"adk.tool.before":       cat(tool, []string{"args"}),
		"adk.tool.after":        cat(tool, []string{"response"}),
		"a2a.request.signed":    {"direction", "wallet", "signed_body_sha256", "target", "message_id", "task_id", "context_id"},
		"a2a.response.received": {"direction", "wallet", "task_state", "task_id", "response_sha256", "target"},
		"a2a.request.accepted":  cat([]string{"outcome", "status", "service", "purpose", "verified", "permission_checked"}, caller, request, serving),
		"a2a.verify.failed":     cat([]string{"outcome", "status", "stage", "wallet", "service"}, verify, request, serving),
		"a2a.permission.denied": cat([]string{"outcome", "status", "stage", "reason", "check_ms"}, caller, check, request, serving),
		"a2a.permission.error":  cat([]string{"outcome", "status", "stage", "reason", "error", "error_type", "check_ms"}, caller, check, request, serving),
	}
}()

// encodeAuditRecord writes an audit record as the Python SDK's
// json.dumps({"event", "ts", **fields}) does: its key order, ", " and ": "
// separators, non-ASCII escaped, empty fields left out.
func encodeAuditRecord(event string, at time.Time, fields map[string]any) (string, error) {
	keys := []string{}
	seen := map[string]bool{}
	for _, k := range auditKeyOrder[event] {
		if v, ok := fields[k]; ok && !emptyField(v) && !seen[k] {
			keys, seen[k] = append(keys, k), true
		}
	}
	var rest []string
	for k, v := range fields {
		if !seen[k] && !emptyField(v) {
			rest = append(rest, k)
		}
	}
	slices.Sort(rest)
	keys = append(keys, rest...)

	var raw bytes.Buffer
	enc := json.NewEncoder(&raw)
	enc.SetEscapeHTML(false)
	raw.WriteString(`{"event":`)
	_ = enc.Encode(event)
	raw.WriteString(`,"ts":`)
	_ = enc.Encode(at.UTC().Format("2006-01-02T15:04:05.000Z"))
	for _, k := range keys {
		raw.WriteByte(',')
		_ = enc.Encode(k)
		raw.WriteByte(':')
		if err := enc.Encode(fields[k]); err != nil {
			return "", err
		}
	}
	raw.WriteByte('}')
	out, err := client.PythonDumps(raw.Bytes(), false)
	return string(out), err
}

// gatePurposes are the purposes SecureA2A's service gate accepts, probed in
// this order, as the Python SDK's WeilA2AMiddleware does.
var gatePurposes = []string{secured.PurposeExecution, secured.PurposeManagement}

// ── Server: weiladk.SecureA2A, the twin of to_secure_a2a / secure_a2a_app ─

// SecureA2A guards an A2A HTTP handler serving agent — typically
// a2asrv.NewJSONRPCHandler(a2asrv.NewHandler(executor)) — the way the
// Python SDK's WeilA2AMiddleware does. For every POST it:
//
//  1. verifies the caller's signed headers, bound to the A2A message, and
//     refuses a reused nonce or a request with no caller (401 otherwise);
//  2. with WithService, checks the caller holds the Execution or Management
//     purpose on chain (403), and audits which one let it in;
//  3. runs WithAuthorize, if given (403);
//  4. serves the request with the caller in context, for the executor
//     (NewExecutor) and the agent's hooks and audits;
//
// and audits each outcome. The agent's identity is the one bound to it
// (Bind / NewLLMAgent), or WithServerIdentity.
func SecureA2A(next http.Handler, served agent.Agent, opts ...A2AOption) (http.Handler, error) {
	o := newA2AOptions(opts)
	id := o.identity
	if id == nil && served != nil {
		id = boundTo(served)
	}
	if id == nil {
		return nil, fmt.Errorf("%w: bind the served agent, or pass WithServerIdentity", ErrNoIdentity)
	}
	if o.service != "" {
		if _, err := secured.ParseScope(o.service); err != nil {
			return nil, err
		}
	}
	// The gate only reads permissions, as the Python SDK's does; verdicts
	// are published by the secured tools themselves.
	gate := o.checker
	if gate == nil {
		gate = secured.NewChecker(secured.UsingSentinel(id.client.Sentinel()), secured.WithoutVerdicts())
	}
	serving := map[string]any{"agent_wallet": id.Address()}
	if served != nil {
		serving["agent"] = served.Name()
	}
	nonces := newNonceSet(o.maxAge)

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			next.ServeHTTP(w, r)
			return
		}
		body, err := io.ReadAll(r.Body)
		r.Body.Close()
		if err != nil {
			writePlain(w, http.StatusBadRequest, "could not read request")
			return
		}
		r.Body = io.NopCloser(bytes.NewReader(body))
		signed := client.SignedBody(body)
		request := requestFields(r, body, signed)
		headerWallet := r.Header.Get(client.HeaderWalletAddress)

		var caller *Caller
		if o.skipVerify {
			if headerWallet == "" {
				var missing []string
				for _, name := range []string{client.HeaderWalletAddress, client.HeaderSignature, client.HeaderMessage, client.HeaderTimestamp} {
					if r.Header.Get(name) == "" {
						missing = append(missing, strings.ToLower(name))
					}
				}
				err = &client.AuthError{Reason: "missing_auth_headers", Detail: map[string]any{"missing_headers": missing}}
			} else {
				caller = unverifiedCaller(r.Header)
			}
		} else {
			caller, err = client.VerifyAuthRequest(r.Header, signed, o.maxAge)
			if err == nil && !nonces.add(caller.Nonce) {
				err = &client.AuthError{Reason: "nonce_reused"}
			}
		}
		if err != nil {
			var shownWallet any
			if headerWallet != "" {
				shownWallet = headerWallet
			}
			fields := merge(map[string]any{"outcome": "rejected", "status": 401, "stage": "verify",
				"wallet": shownWallet, "service": o.service}, verifyFailure(r.Header, signed, err))
			o.audit(id, "a2a.verify.failed", merge(fields, request, serving))
			shown := headerWallet
			if shown == "" {
				shown = "<missing X-Wallet-Address>"
			}
			writePlain(w, http.StatusUnauthorized, fmt.Sprintf("Wallet address verification failed for wallet '%s'", shown))
			return
		}
		callerFields := map[string]any{"wallet": caller.Address}
		if caller.Canonical {
			callerFields["wallet_source"] = "signed_canonical"
			callerFields["wallet_header"] = headerWallet
		} else {
			callerFields["wallet_source"] = "header"
		}

		ctx := secured.ContextWithCaller(context.WithValue(r.Context(), servedKey{}, true), caller)
		if o.checker != nil {
			ctx = secured.ContextWithChecker(ctx, o.checker)
		}
		granted := ""
		if o.service != "" {
			check := map[string]any{"service": o.service, "permission_check": "secured.key_has_purpose",
				"required_purposes": gatePurposes, "sentinel_host": id.client.SentinelHost()}
			wallet := caller.Address
			started := time.Now()
			granted, err = gate.Grant(ctx, wallet, o.service, gatePurposes...)
			elapsed := time.Since(started).Milliseconds()
			var npe *secured.NotPermittedError
			switch {
			case errors.As(err, &npe):
				o.audit(id, "a2a.permission.denied", merge(map[string]any{
					"outcome": "denied", "status": 403, "stage": "permission",
					"reason": fmt.Sprintf("wallet holds neither the %s nor the %s purpose on service %s",
						secured.PurposeExecution, secured.PurposeManagement, o.service),
					"check_ms": elapsed,
				}, callerFields, check, request, serving))
				writePlain(w, http.StatusForbidden, err.Error())
				return
			case err != nil:
				cause := rootCause(err)
				o.audit(id, "a2a.permission.error", merge(map[string]any{
					"outcome": "error", "status": 403, "stage": "permission",
					"reason": "secured permission check could not be completed",
					"error":  cause.Error(), "error_type": errorType(cause), "check_ms": elapsed,
				}, callerFields, check, request, serving))
				writePlain(w, http.StatusForbidden, fmt.Sprintf("Wallet '%s' failed the secured permission check for service '%s': %v", wallet, o.service, cause))
				return
			}
		}
		if o.authorize != nil {
			if err := o.authorize(ContextWithIdentity(ctx, id), caller); err != nil {
				o.audit(id, "a2a.permission.denied", merge(map[string]any{
					"outcome": "denied", "status": 403, "stage": "authorize", "reason": err.Error(),
				}, callerFields, request, serving))
				writePlain(w, http.StatusForbidden, "caller not authorized: "+err.Error())
				return
			}
		}

		accepted := map[string]any{"outcome": "accepted", "status": 200, "service": o.service,
			"verified": !o.skipVerify, "permission_checked": o.service != "",
			// The purpose that let the caller in ("" without a service gate).
			"purpose": granted}
		o.audit(id, "a2a.request.accepted", merge(accepted, callerFields, request, serving))
		next.ServeHTTP(w, r.WithContext(ctx))
	}), nil
}

// ── Executor: weiladk.NewExecutor, the twin of adka2a.NewExecutor ────────

// NewExecutor is adka2a.NewExecutor for agents served behind SecureA2A: the
// same ExecutorConfig. It refuses requests that did not come through
// SecureA2A, hands the verified caller to the agent's hooks (Event.Caller)
// and tools (CallerFromContext), and audits each agent run (adk.agent.before / adk.agent.after) with the
// caller that triggered it.
func NewExecutor(cfg adka2a.ExecutorConfig, opts ...A2AOption) (*adka2a.Executor, error) {
	o := newA2AOptions(opts)
	id := o.identity
	if id == nil && cfg.RunnerConfig.Agent != nil {
		id = boundTo(cfg.RunnerConfig.Agent)
	}
	if id == nil {
		return nil, fmt.Errorf("%w: bind the served agent, or pass WithServerIdentity", ErrNoIdentity)
	}

	next := cfg.BeforeExecuteCallback
	cfg.BeforeExecuteCallback = func(ctx context.Context, execCtx *a2asrv.ExecutorContext) (context.Context, error) {
		if _, ok := secured.CallerFromContext(ctx); !ok {
			return nil, errors.New("weiladk: request has no verified caller: serve the executor behind weiladk.SecureA2A")
		}
		if next != nil {
			return next(ctx, execCtx)
		}
		return ctx, nil
	}

	// Python's setup_secure_a2a on the served agent: agent and secured-tool
	// audits (unless WithoutAudit) and run traces. It goes first, so a user
	// plugin that short-circuits a hook cannot hide the audit.
	p, err := NewPlugin(plugin.Config{Name: "weil_a2a"},
		WithSecureA2A(SecureA2AConfig{Service: o.service, DisableAudit: o.noAudit || o.noAgentAudit}),
		WithFallbackIdentity(Static(id)),
		WithDispatcher(o.dispatcher))
	if err != nil {
		return nil, err
	}
	cfg.RunnerConfig.PluginConfig.Plugins = append([]*plugin.Plugin{p}, cfg.RunnerConfig.PluginConfig.Plugins...)
	return adka2a.NewExecutor(cfg), nil
}

// AgentAuditAction records each local agent run as the Python SDK does
// (adk.agent.before / adk.agent.after events), with the A2A caller that
// triggered it. NewExecutor installs it; add it to a calling agent's plugin
// to audit that side too.
func AgentAuditAction(service string) Action {
	return Action{
		Name:  "a2a_agent_audit",
		Hooks: []Hook{HookBeforeAgent, HookAfterAgent},
		// Inline, as Python awaits it; the audit call itself is fire and
		// forget, and a failure never fails the hook (FailOpen).
		Mode: Blocking,
		Build: func(ev *Event) (*Call, error) {
			a := actingAgent(ev.Context())
			// Remote A2A agents are audited by their own server.
			if _, remote := remoteIdentities.Load(a); remote || agentType(a) == "RemoteA2aAgent" {
				return nil, nil
			}
			event := "adk.agent.before"
			if ev.Hook == HookAfterAgent {
				event = "adk.agent.after"
			}
			// Only the root agent is served under the service scope.
			svc := ""
			// Names are unique within an agent tree.
			if root, ok := runRoots.Load(ev.InvocationID); !ok || root.(agent.Agent).Name() == ev.AgentName {
				svc = service
			}
			fields := map[string]any{
				"agent": ev.AgentName, "agent_type": agentType(a), "service": svc,
			}
			if ev.Identity != nil {
				fields["wallet"] = ev.Identity.Address()
			}
			if ev.Caller != nil && (ev.Identity == nil || ev.Caller.Address != ev.Identity.Address()) {
				fields["caller_wallet"] = ev.Caller.Address
			}
			return auditCall(ev.Identity, event, fields)
		},
	}
}

// ── Audit records ────────────────────────────────────────────────────────
//
// Only metadata — addresses, ids, sizes, hashes, outcomes. Message parts,
// signatures and the signed X-Message never go to the immutable audit log.

func (o *a2aOptions) audit(id *Identity, event string, fields map[string]any) {
	if o.noAudit || id == nil {
		return
	}
	call, err := auditCall(id, event, fields)
	if err != nil {
		return
	}
	// In the background, waiting for the auditor to run it (submitAudit):
	// the request does not wait on the chain, and an entry the platform
	// loses is resent rather than gone.
	submitAudit(id, *call, func(err error) {
		slog.Warn("weiladk a2a audit failed", "event", event, "wallet", id.Address(), "error", err)
	})
}

func auditCall(id *Identity, event string, fields map[string]any) (*Call, error) {
	return auditCallAt(id, event, fields, time.Now())
}

func auditCallAt(id *Identity, event string, fields map[string]any, at time.Time) (*Call, error) {
	log, err := encodeAuditRecord(event, at, fields)
	if err != nil {
		return nil, err
	}
	var org *wallet.OrgInfo
	if id != nil {
		org = id.Org()
	}
	// Arguments public, as the Python SDK submits audits. The call waits for
	// the auditor to run the entry: a submission the platform accepted can
	// still be lost before it runs, and only waiting shows it.
	return &Call{Applet: Auditor, Method: "audit", Args: pyArgs(client.AuditArgs(string(log), org)), Retry: client.DurableRetry}, nil
}

func emptyField(v any) bool {
	switch x := v.(type) {
	case nil:
		return true
	case string:
		return x == ""
	case []string:
		return len(x) == 0
	case map[string]any:
		return x == nil // Python's None
	}
	return false
}

func merge(maps ...map[string]any) map[string]any {
	out := map[string]any{}
	for _, m := range maps {
		for k, v := range m {
			out[k] = v
		}
	}
	return out
}

func shortHash(h string) string {
	if len(h) > 16 {
		return h[:16]
	}
	return h
}

// messageFields extracts correlation ids from an A2A message (camelCase or
// snake_case).
func messageFields(signed any) map[string]any {
	raw, ok := signed.(json.RawMessage)
	if !ok {
		return nil
	}
	var m map[string]any
	if json.Unmarshal(raw, &m) != nil {
		return nil
	}
	pick := func(keys ...string) any {
		for _, k := range keys {
			if v, ok := m[k].(string); ok && v != "" {
				return v
			}
		}
		return nil
	}
	return map[string]any{
		"message_id": pick("messageId", "message_id"),
		"task_id":    pick("taskId", "task_id"),
		"context_id": pick("contextId", "context_id"),
	}
}

func requestFields(r *http.Request, body []byte, signed any) map[string]any {
	f := map[string]any{"path": r.URL.Path, "user_agent": r.UserAgent()}
	if host, _, err := net.SplitHostPort(r.RemoteAddr); err == nil {
		f["peer"] = host
	}
	if len(body) > 0 {
		f["body_bytes"] = len(body)
	}
	if sum, err := client.BodySHA256(signed); err == nil {
		f["body_sha256"] = shortHash(sum)
	}
	var envelope struct {
		Method string `json:"method"`
		ID     any    `json:"id"`
	}
	if json.Unmarshal(body, &envelope) == nil {
		// Empty or zero values are left out, as Python's `or None` does.
		if envelope.Method != "" {
			f["rpc_method"] = envelope.Method
		}
		if envelope.ID != nil && envelope.ID != "" && envelope.ID != float64(0) && envelope.ID != false {
			f["rpc_id"] = envelope.ID
		}
	}
	return merge(f, messageFields(signed))
}

// responseFields describes a remote reply the way the Python SDK audits it:
// the final task state, the task id and a digest to later prove a stored
// reply is the one received. ADK hands the callback a session event, not the
// A2A task, so the state is "failed" for an error and "completed" otherwise,
// and the digest is of the reply's content.
func responseFields(resp *session.Event, err error) map[string]any {
	f := map[string]any{"task_state": "completed"}
	if err != nil || (resp != nil && resp.ErrorMessage != "") {
		f["task_state"] = "failed"
	}
	if resp == nil {
		return f
	}
	if resp.Content != nil {
		f["response_sha256"] = shortHash(HashJSON(resp.Content))
	}
	if v, ok := resp.CustomMetadata["a2a:task_id"]; ok {
		f["task_id"] = v
	}
	return f
}

// nonceSet accepts each nonce once while its headers can still be fresh.
type nonceSet struct {
	ttl  time.Duration
	mu   sync.Mutex
	seen map[string]time.Time
}

func newNonceSet(maxAge time.Duration) *nonceSet {
	if maxAge <= 0 {
		maxAge = client.MaxAuthAge
	}
	return &nonceSet{ttl: 2 * maxAge, seen: map[string]time.Time{}}
}

func (n *nonceSet) add(nonce string) bool {
	if nonce == "" {
		return true
	}
	n.mu.Lock()
	defer n.mu.Unlock()
	now := time.Now()
	for k, at := range n.seen {
		if now.Sub(at) > n.ttl {
			delete(n.seen, k)
		}
	}
	if _, ok := n.seen[nonce]; ok {
		return false
	}
	n.seen[nonce] = now
	return true
}
