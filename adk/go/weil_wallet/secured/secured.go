// Package secured gates services on WeilChain permissions — the Go twin of
// the Python SDK's weil_ai secured_user / secured_admin and weil_middleware.
//
// A caller proves its wallet with signed auth headers (client.AuthHeaders);
// Middleware verifies them and puts the caller in the request context. A
// gate (Require, or the MCP and ADK wrappers built on it) then asks the
// org's identity applet (identity::<org>) key_has_purpose for the purpose the
// scope demands:
//
//	"engg.weil"  → subgroup "engg" of org "weil": key "engg_<wallet>"
//	"weil"       → org-level: key "<wallet>"
//
// User level accepts the Execution or Management purpose, Admin level only
// Management. As in the on-chain #[secured] macro, an org-level key also
// satisfies a subgroup scope (see StrictSubgroups).
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
	// User accepts keys with the Execution or Management purpose
	// (Python: secured_user).
	User Level = iota
	// Admin accepts only keys with the Management purpose
	// (Python: secured_admin).
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

// Scope is a parsed "subgroup.org" or "org" service scope.
type Scope struct {
	Org      string
	Subgroup string
}

// ParseScope parses "subgroup.org" or "org". It is not an applet name: the
// org's identity applet is identity::<org>.
func ParseScope(s string) (Scope, error) {
	v := strings.TrimSpace(s)
	switch {
	case v == "":
		return Scope{}, errors.New("secured: scope must not be empty")
	case strings.Contains(v, "::"):
		return Scope{}, fmt.Errorf(`secured: scope %q must use dot syntax, e.g. "subgroup.org"`, s)
	case !strings.Contains(v, "."):
		return Scope{Org: v}, nil
	}
	subgroup, org, _ := strings.Cut(v, ".")
	if subgroup == "" || org == "" {
		return Scope{}, fmt.Errorf(`secured: scope %q must be "subgroup.org" or "org"`, s)
	}
	return Scope{Org: org, Subgroup: subgroup}, nil
}

func (s Scope) String() string {
	if s.Subgroup == "" {
		return s.Org
	}
	return s.Subgroup + "." + s.Org
}

// IdentityApplet is the service name of the org's identity applet.
func (s Scope) IdentityApplet() string { return "identity::" + s.Org }

// PublisherApplet is the service name of the org's publisher applet, where
// secured-check verdicts go.
func (s Scope) PublisherApplet() string { return "publisher::" + s.Org }

// Key is the identity-applet key of wallet within the scope.
func (s Scope) Key(wallet string) string {
	if s.Subgroup == "" {
		return wallet
	}
	return s.Subgroup + "_" + wallet
}

// ErrNotPermitted is matched by every *NotPermittedError.
var ErrNotPermitted = errors.New("wallet not permitted")

// NotPermittedError is returned when a caller lacks the permission a gate
// demands (Python: WalletNotPermittedError, same message).
type NotPermittedError struct {
	Wallet  string
	Service string
	Level   Level
}

func (e *NotPermittedError) Error() string {
	return fmt.Sprintf("wallet '%s' does not have permission to call tools secured by applet '%s'", e.Wallet, e.Service)
}

func (e *NotPermittedError) Is(target error) bool { return target == ErrNotPermitted }

// Require checks that the verified caller in ctx (see Middleware) holds the
// permission scope and level demand, using the Checker in ctx (or the
// default one). It is the gate secured_user / secured_admin run before a
// tool; a missing caller is denied.
func Require(ctx context.Context, scope string, level Level) error {
	caller, ok := CallerFromContext(ctx)
	if !ok {
		return &NotPermittedError{Service: scope, Level: level}
	}
	return checkerFrom(ctx).Check(ctx, caller.Address, scope, level)
}
