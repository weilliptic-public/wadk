package secured

import (
	"context"
	"errors"
	"fmt"
	"sort"
	"strings"
	"sync"
)

// A Rule is a boolean access rule over scopes, as taken by the gates and
// the Python SDK's @secured_user / @secured_admin (weil_ai.secured_expr). It
// combines scopes with ! (not), & (and) and | (or). ! binds tightest, then
// &, then |; & and | are left-associative and parentheses override:
//
//	rule     :=  and_part ( "|" and_part )*
//	and_part :=  atom     ( "&" atom )*
//	atom     :=  "!" atom  |  "(" rule ")"  |  name
//
// A name is "subgroup.org" or "org", bare (engg.weil) or quoted
// ("engg.weil"). The last dot-segment is the org; any segments before it
// are the subgroup, joined by "_": "a.b.weil" is subgroup a_b in org weil.
//
// Evaluation is three-valued (see Outcome): a check that could not be
// completed is Unknown, which never grants access, not even under !.
type Rule interface {
	isRule()
	String() string
}

// Atom is one "subgroup.org" or "org" condition.
type Atom struct {
	Literal  string
	Org      string
	Subgroup string
}

// Not negates a rule.
type Not struct{ Inner Rule }

// And holds when both sides do.
type And struct{ LHS, RHS Rule }

// Or holds when either side does.
type Or struct{ LHS, RHS Rule }

func (Atom) isRule() {}
func (Not) isRule()  {}
func (And) isRule()  {}
func (Or) isRule()   {}

func (a Atom) String() string { return a.Literal }
func (n Not) String() string  { return "!" + n.Inner.String() }
func (a And) String() string  { return "(" + a.LHS.String() + " & " + a.RHS.String() + ")" }
func (o Or) String() string   { return "(" + o.LHS.String() + " | " + o.RHS.String() + ")" }

// ErrBadRule is matched by every error ParseRule returns.
var ErrBadRule = errors.New("secured: bad rule")

type ruleError struct{ msg string }

func (e *ruleError) Error() string        { return "secured: " + e.msg }
func (e *ruleError) Is(target error) bool { return target == ErrBadRule }

func badRule(format string, args ...any) error {
	return &ruleError{fmt.Sprintf(format, args...)}
}

// SplitLiteral splits a scope at its last dot: "engg.team.weil" is org
// "weil", subgroup "engg_team"; "weil" is org "weil", no subgroup.
func SplitLiteral(literal string) (org, subgroup string) {
	items := strings.Split(literal, ".")
	return items[len(items)-1], strings.Join(items[:len(items)-1], "_")
}

func newAtom(literal string) (Atom, error) {
	v := strings.TrimSpace(literal)
	if v == "" {
		return Atom{}, badRule("scope must not be empty")
	}
	for _, part := range strings.Split(v, ".") {
		if part == "" {
			return Atom{}, badRule(`scope %q must be "subgroup.org" or "org" for org-level access`, v)
		}
	}
	org, subgroup := SplitLiteral(v)
	return Atom{Literal: v, Org: org, Subgroup: subgroup}, nil
}

type token struct {
	op   string // "", or one of ( ) & | !
	name string
}

func isNameByte(c byte) bool {
	return c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' ||
		c == '_' || c == '-' || c == '.' || c == ':'
}

func tokenize(rule string) ([]token, error) {
	var tokens []token
	for i := 0; i < len(rule); {
		c := rule[i]
		switch {
		case c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f' || c == '\v':
			i++
		case strings.HasPrefix(rule[i:], "||"), strings.HasPrefix(rule[i:], "&&"), strings.HasPrefix(rule[i:], "!="):
			return nil, badRule("unsupported operator %q in secured rule %q — use single '&', '|' and '!'", rule[i:i+2], rule)
		case strings.IndexByte("()&|!", c) >= 0:
			tokens = append(tokens, token{op: string(c)})
			i++
		case c == '"':
			end := strings.IndexByte(rule[i+1:], '"')
			if end < 0 {
				return nil, badRule("unexpected character '\"' in secured rule %q", rule)
			}
			tokens = append(tokens, token{name: rule[i+1 : i+1+end]})
			i += end + 2
		case isNameByte(c):
			j := i
			for j < len(rule) && isNameByte(rule[j]) {
				j++
			}
			tokens = append(tokens, token{name: rule[i:j]})
			i = j
		default:
			return nil, badRule("unexpected character %q in secured rule %q", string(c), rule)
		}
	}
	return tokens, nil
}

type parser struct {
	rule   string
	tokens []token
	pos    int
}

func (p *parser) peek() *token {
	if p.pos < len(p.tokens) {
		return &p.tokens[p.pos]
	}
	return nil
}

func (p *parser) peekOp(op string) bool {
	t := p.peek()
	return t != nil && t.op == op
}

func (p *parser) errorf(format string, args ...any) error {
	return badRule("%s in secured rule %q", fmt.Sprintf(format, args...), p.rule)
}

func (p *parser) or() (Rule, error) {
	lhs, err := p.and()
	for err == nil && p.peekOp("|") {
		p.pos++
		var rhs Rule
		if rhs, err = p.and(); err == nil {
			lhs = Or{lhs, rhs}
		}
	}
	return lhs, err
}

func (p *parser) and() (Rule, error) {
	lhs, err := p.atom()
	for err == nil && p.peekOp("&") {
		p.pos++
		var rhs Rule
		if rhs, err = p.atom(); err == nil {
			lhs = And{lhs, rhs}
		}
	}
	return lhs, err
}

func (p *parser) atom() (Rule, error) {
	t := p.peek()
	switch {
	case t == nil:
		return nil, p.errorf("expected a scope, '!' or '(' but the rule ended")
	case t.op == "!":
		p.pos++
		inner, err := p.atom()
		if err != nil {
			return nil, err
		}
		return Not{inner}, nil
	case t.op == "(":
		p.pos++
		inner, err := p.or()
		if err != nil {
			return nil, err
		}
		if !p.peekOp(")") {
			return nil, p.errorf("expected ')'")
		}
		p.pos++
		return inner, nil
	case t.op == "":
		p.pos++
		return newAtom(t.name)
	}
	return nil, p.errorf("expected a scope, '!' or '(' but found %q", t.op)
}

var ruleCache sync.Map // string → Rule

// ParseRule parses a secured rule such as "infrastructure.weil &
// !finance.weil". Errors match ErrBadRule.
func ParseRule(rule string) (Rule, error) {
	if r, ok := ruleCache.Load(rule); ok {
		return r.(Rule), nil
	}
	tokens, err := tokenize(rule)
	if err != nil {
		return nil, err
	}
	if len(tokens) == 0 {
		return nil, badRule("scope must not be empty")
	}
	p := &parser{rule: rule, tokens: tokens}
	r, err := p.or()
	if err != nil {
		return nil, err
	}
	if t := p.peek(); t != nil {
		return nil, p.errorf("unexpected %q", t.op+t.name)
	}
	ruleCache.Store(rule, r)
	return r, nil
}

// Orgs is every org named anywhere in r, sorted.
func Orgs(r Rule) []string {
	seen := map[string]bool{}
	var walk func(Rule)
	walk = func(n Rule) {
		switch n := n.(type) {
		case Atom:
			seen[n.Org] = true
		case Not:
			walk(n.Inner)
		case And:
			walk(n.LHS)
			walk(n.RHS)
		case Or:
			walk(n.LHS)
			walk(n.RHS)
		}
	}
	walk(r)
	orgs := make([]string, 0, len(seen))
	for org := range seen {
		orgs = append(orgs, org)
	}
	sort.Strings(orgs)
	return orgs
}

// Outcome is the result of one check or a whole rule: allowed, denied or
// undecided.
type Outcome int8

const (
	// Unknown: a check could not be completed.
	Unknown Outcome = iota
	False
	True
)

func (o Outcome) String() string {
	switch o {
	case True:
		return "true"
	case False:
		return "false"
	}
	return "unknown"
}

// OutcomeOf is True or False.
func OutcomeOf(b bool) Outcome {
	if b {
		return True
	}
	return False
}

// AndOutcome is three-valued &: False wins, then Unknown, then True.
func AndOutcome(lhs, rhs Outcome) Outcome {
	switch {
	case lhs == False || rhs == False:
		return False
	case lhs == True && rhs == True:
		return True
	}
	return Unknown
}

// OrOutcome is three-valued |: True wins, then Unknown, then False.
func OrOutcome(lhs, rhs Outcome) Outcome {
	switch {
	case lhs == True || rhs == True:
		return True
	case lhs == False && rhs == False:
		return False
	}
	return Unknown
}

// NotOutcome is three-valued !: Unknown stays Unknown.
func NotOutcome(inner Outcome) Outcome {
	switch inner {
	case True:
		return False
	case False:
		return True
	}
	return Unknown
}

// Evaluate evaluates r, calling check(org, subgroup) for each condition.
// The right side of & and | is evaluated only when the left side has not
// already decided the result.
func Evaluate(ctx context.Context, r Rule, check func(ctx context.Context, org, subgroup string) Outcome) Outcome {
	switch n := r.(type) {
	case Atom:
		return check(ctx, n.Org, n.Subgroup)
	case Not:
		return NotOutcome(Evaluate(ctx, n.Inner, check))
	case And:
		lhs := Evaluate(ctx, n.LHS, check)
		if lhs == False {
			return False
		}
		return AndOutcome(lhs, Evaluate(ctx, n.RHS, check))
	case Or:
		lhs := Evaluate(ctx, n.LHS, check)
		if lhs == True {
			return True
		}
		return OrOutcome(lhs, Evaluate(ctx, n.RHS, check))
	}
	return Unknown
}

// EvaluateRootFirst evaluates r for wallet, org-level access first, as the
// Python SDK and the on-chain macros do:
//
//  1. For every org r names (sorted), wallet's org-level key — the bare
//     address — is checked once, up front.
//  2. If that is True for every org, r is not evaluated at all: True.
//  3. Otherwise each condition is org-level OR subgroup key: "engg.weil"
//     passes on the org-level weil key, else on "engg_<wallet>". An
//     org-only condition ("weil") is just the org-level result.
//
// checkKey(org, key) asks whether key holds the required purpose on org's
// identity applet.
func EvaluateRootFirst(ctx context.Context, r Rule, wallet string, checkKey func(ctx context.Context, org, key string) Outcome) Outcome {
	return evaluateRootFirst(ctx, r, wallet, false, checkKey)
}

// evaluateRootFirst with strict set ignores org-level keys for subgroup
// conditions (StrictSubgroups).
func evaluateRootFirst(ctx context.Context, r Rule, wallet string, strict bool, checkKey func(ctx context.Context, org, key string) Outcome) Outcome {
	roots := map[string]Outcome{}
	if !strict {
		all := true
		for _, org := range Orgs(r) {
			roots[org] = checkKey(ctx, org, wallet)
			all = all && roots[org] == True
		}
		// Org-level access passes without evaluating the rule.
		if all && len(roots) > 0 {
			return True
		}
	}
	return Evaluate(ctx, r, func(ctx context.Context, org, subgroup string) Outcome {
		if strict && subgroup != "" {
			return checkKey(ctx, org, subgroup+"_"+wallet)
		}
		root, ok := roots[org]
		if !ok {
			root = checkKey(ctx, org, wallet)
			roots[org] = root
		}
		if subgroup == "" || root == True {
			return root
		}
		return OrOutcome(root, checkKey(ctx, org, subgroup+"_"+wallet))
	})
}
