// Package secured gates services on WeilChain permissions — the Go twin of
// the Python SDK's weil_ai secured_user / secured_admin and weil_middleware.
//
// A caller proves its wallet with signed auth headers (client.AuthHeaders);
// Middleware verifies them and puts the caller in the request context. A
// gate (Require, or the MCP and ADK wrappers built on it) then asks each
// org's identity applet (identity::<org>) key_has_purpose for the purpose
// the rule demands:
//
//	"engg.weil"  → subgroup "engg" of org "weil": key "engg_<wallet>"
//	"weil"       → org-level: key "<wallet>"
//
// A rule is one scope or a boolean Rule over several, e.g.
// "(engg.weil | legal.weil) & !contractors.weil" — the same language and
// evaluation as Python's weil_ai.secured_expr and the Rust macros.
//
// User level accepts the Execution or Management purpose, Admin level only
// Management. As in the on-chain #[secured] macro, an org-level key also
// satisfies a subgroup condition, and a wallet with org-level access in
// every org a rule names passes without the rule being evaluated (see
// StrictSubgroups).
package secured

import (
	"context"
	"errors"
	"fmt"
	"strings"
)

// Level is the access level a gate demands.
type Level int

const (
	// User accepts keys with the Execution or Management purpose.
	User Level = iota
	// Admin accepts only keys with the Management purpose.
	Admin
)

func (l Level) String() string {
	if l == Admin {
		return "admin"
	}
	return "user"
}

// Key purposes held on an org's identity applet.
const (
	PurposeExecution  = "Execution"
	PurposeManagement = "Management"
)

func (l Level) purposes() []string {
	if l == Admin {
		return []string{PurposeManagement}
	}
	// Management first: it satisfies both levels, so an admin key costs one
	// round trip instead of two.
	return []string{PurposeManagement, PurposeExecution}
}

// ValidateRule reports whether rule is a valid gate rule: it parses, and
// uses dot syntax ("subgroup.org", never an applet name like
// "identity::weil"). Gates call it when they are built, so a malformed rule
// fails then rather than on the first call. Errors match ErrBadRule.
func ValidateRule(rule string) error {
	_, err := parseGateRule(rule)
	return err
}

func parseGateRule(rule string) (Rule, error) {
	if strings.Contains(rule, "::") {
		return nil, badRule(`scope %q must use dot syntax, e.g. "subgroup.org"`, rule)
	}
	return ParseRule(rule)
}

// identityApplet is the service name of org's identity applet, which holds
// the keys and their purposes.
func identityApplet(org string) string { return "identity::" + org }

// publisherApplet is the service name of org's publisher applet, where
// secured-check verdicts go.
func publisherApplet(org string) string { return "publisher::" + org }

// ErrNotPermitted is matched by every *NotPermittedError.
var ErrNotPermitted = errors.New("wallet not permitted")

// NotPermittedError is returned when a caller lacks the permission a gate
// demands. Its message is the one the other Weil SDKs give.
type NotPermittedError struct {
	Wallet  string
	Service string
	Level   Level
}

func (e *NotPermittedError) Error() string {
	return fmt.Sprintf("wallet '%s' does not have permission to call tools secured by applet '%s'", e.Wallet, e.Service)
}

func (e *NotPermittedError) Is(target error) bool { return target == ErrNotPermitted }

// Require checks that the verified caller in ctx (see Middleware) satisfies
// rule at level, using the Checker in ctx (or the
// default one). It is the gate secured_user / secured_admin run before a
// tool; a missing caller is denied.
func Require(ctx context.Context, rule string, level Level) error {
	caller, ok := CallerFromContext(ctx)
	if !ok {
		return &NotPermittedError{Service: rule, Level: level}
	}
	return checkerFrom(ctx).Check(ctx, caller.Address, rule, level)
}
