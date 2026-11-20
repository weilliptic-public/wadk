"""Boolean access rules for ``@secured_user`` / ``@secured_admin``.

A rule combines scopes with ``!`` (not), ``&`` (and) and ``|`` (or). ``!``
binds tightest, then ``&``, then ``|``; ``&``/``|`` are left-associative and
parentheses override::

    rule     :=  and_part ( "|" and_part )*
    and_part :=  atom     ( "&" atom )*
    atom     :=  "!" atom  |  "(" rule ")"  |  name

A *name* is ``"subgroup.org"`` or just ``"org"`` for org-level, written bare
(``engg.weil``) or quoted (``"engg.weil"``). The last ``.``-segment is the
org; any segments before it are the subgroup, joined by ``_`` —
``"a.b.weil"`` is subgroup ``a_b`` in org ``weil``.

Evaluation is three-valued: ``True``, ``False``, or ``None`` when a check
could not be completed (missing identity applet, failed call). ``None`` is
not ``False``: it never blocks access through an ``|`` alternative that is
fine, and ``!`` of ``None`` stays ``None``, so a broken lookup can never be
what grants access. Access is granted only when the whole rule is ``True``.
The right side of ``&``/``|`` is evaluated only when the left side has not
already decided the result.

Worked examples, rule by rule, are in ``tests/test_secured_examples.py``.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from functools import lru_cache
from typing import Awaitable, Callable, Optional, Union

Outcome = Optional[bool]
"""Result of one check or a whole rule: True, False, or None (unknown)."""


class SecuredExprError(ValueError):
    """Raised when a secured rule cannot be parsed."""


@dataclass(frozen=True)
class Atom:
    """One ``"subgroup.org"`` (or ``"org"``) condition."""

    literal: str
    org: str
    subgroup: str = ""


@dataclass(frozen=True)
class Not:
    inner: "SecuredExpr"


@dataclass(frozen=True)
class And:
    lhs: "SecuredExpr"
    rhs: "SecuredExpr"


@dataclass(frozen=True)
class Or:
    lhs: "SecuredExpr"
    rhs: "SecuredExpr"


SecuredExpr = Union[Atom, Not, And, Or]

_TOKEN_RE = re.compile(
    r"""
    \s*(?:
        (?P<op>\|\||&&|!=|[()&|!])          # operators (|| && != rejected below)
      | "(?P<quoted>[^"]*)"                  # "engg.weil"
      | (?P<bare>[A-Za-z0-9_\-.:]+)          # engg.weil
    )
    """,
    re.VERBOSE,
)


def split_secured_literal(literal: str) -> tuple[str, str]:
    """``"engg.team.weil"`` → ``("weil", "engg_team")``; ``"weil"`` → ``("weil", "")``.

    """
    items = literal.split(".")
    org = items[-1]
    subgroup = "_".join(items[:-1])
    return org, subgroup


def _atom(literal: str) -> Atom:
    value = literal.strip()
    if not value:
        raise SecuredExprError("secured scope must not be empty")
    if any(not part for part in value.split(".")):
        raise SecuredExprError(
            f'secured scope {value!r} must be "subgroup.org" or "org" for org-level access'
        )
    org, subgroup = split_secured_literal(value)
    return Atom(literal=value, org=org, subgroup=subgroup)


def _tokenize(rule: str) -> list[tuple[str, str]]:
    tokens: list[tuple[str, str]] = []
    pos = 0
    while pos < len(rule):
        if rule[pos:].strip() == "":
            break
        match = _TOKEN_RE.match(rule, pos)
        if match is None or match.end() == pos:
            raise SecuredExprError(
                f"unexpected character {rule[pos:].lstrip()[:1]!r} in secured rule {rule!r}"
            )
        if match.group("op") is not None:
            op = match.group("op")
            if op in ("||", "&&", "!="):
                raise SecuredExprError(
                    f"unsupported operator {op!r} in secured rule {rule!r} — "
                    "use single '&', '|' and '!'"
                )
            tokens.append(("op", op))
        elif match.group("quoted") is not None:
            tokens.append(("name", match.group("quoted")))
        else:
            tokens.append(("name", match.group("bare")))
        pos = match.end()
    return tokens


class _Parser:
    def __init__(self, rule: str) -> None:
        self.rule = rule
        self.tokens = _tokenize(rule)
        self.pos = 0

    def _peek(self) -> Optional[tuple[str, str]]:
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def _peek_op(self, op: str) -> bool:
        return self._peek() == ("op", op)

    def _error(self, message: str) -> SecuredExprError:
        return SecuredExprError(f"{message} in secured rule {self.rule!r}")

    def parse(self) -> SecuredExpr:
        if not self.tokens:
            raise SecuredExprError("secured scope must not be empty")
        expr = self._or()
        if self._peek() is not None:
            raise self._error(f"unexpected {self._peek()[1]!r}")
        return expr

    def _or(self) -> SecuredExpr:
        lhs = self._and()
        while self._peek_op("|"):
            self.pos += 1
            lhs = Or(lhs, self._and())
        return lhs

    def _and(self) -> SecuredExpr:
        lhs = self._atom()
        while self._peek_op("&"):
            self.pos += 1
            lhs = And(lhs, self._atom())
        return lhs

    def _atom(self) -> SecuredExpr:
        token = self._peek()
        if token is None:
            raise self._error("expected a scope, '!' or '(' but the rule ended")
        kind, value = token
        if token == ("op", "!"):
            self.pos += 1
            return Not(self._atom())
        if token == ("op", "("):
            self.pos += 1
            inner = self._or()
            if not self._peek_op(")"):
                raise self._error("expected ')'")
            self.pos += 1
            return inner
        if kind == "name":
            self.pos += 1
            return _atom(value)
        raise self._error(f"expected a scope, '!' or '(' but found {value!r}")


@lru_cache(maxsize=256)
def parse_secured_expr(rule: str) -> SecuredExpr:
    """Parse a secured rule such as ``"infrastructure.weil & !finance.weil"``.

    Raises:
        SecuredExprError: the rule is empty or malformed.
    """
    return _Parser(rule).parse()


def secured_orgs(expr: SecuredExpr) -> list[str]:
    """Every org named anywhere in *expr*, sorted."""
    orgs: set[str] = set()

    def walk(node: SecuredExpr) -> None:
        if isinstance(node, Atom):
            orgs.add(node.org)
        elif isinstance(node, Not):
            walk(node.inner)
        else:
            walk(node.lhs)
            walk(node.rhs)

    walk(expr)
    return sorted(orgs)


def outcome_and(lhs: Outcome, rhs: Outcome) -> Outcome:
    """Three-valued ``&``: False wins, then unknown, then True."""
    if lhs is False or rhs is False:
        return False
    if lhs is True and rhs is True:
        return True
    return None


def outcome_or(lhs: Outcome, rhs: Outcome) -> Outcome:
    """Three-valued ``|``: True wins, then unknown, then False."""
    if lhs is True or rhs is True:
        return True
    if lhs is False and rhs is False:
        return False
    return None


def outcome_not(inner: Outcome) -> Outcome:
    """Three-valued ``!``: unknown stays unknown."""
    return None if inner is None else not inner


CheckFn = Callable[[str, str], Awaitable[Outcome]]
"""``check(org, subgroup)`` — does the caller satisfy one condition?"""


async def evaluate_secured_expr(expr: SecuredExpr, check: CheckFn) -> Outcome:
    """Evaluate *expr*, calling ``check(org, subgroup)`` for each condition.

    The right side of ``&``/``|`` runs only when the left side has not
    already decided the result, so its checks are skipped entirely.
    """
    if isinstance(expr, Atom):
        return await check(expr.org, expr.subgroup)
    if isinstance(expr, Not):
        return outcome_not(await evaluate_secured_expr(expr.inner, check))
    lhs = await evaluate_secured_expr(expr.lhs, check)
    if isinstance(expr, And):
        if lhs is False:
            return False
        return outcome_and(lhs, await evaluate_secured_expr(expr.rhs, check))
    if lhs is True:
        return True
    return outcome_or(lhs, await evaluate_secured_expr(expr.rhs, check))


async def evaluate_root_first(
    expr: SecuredExpr,
    check_key: Callable[[str, str], Awaitable[Outcome]],
    wallet_addr: str,
) -> Outcome:
    """Evaluate *expr* for *wallet_addr*, org-level access first.

    1. For every org named in the rule (sorted), the caller's org-level key —
       the bare wallet address — is checked once, up front.
    2. If that check is a definite yes for every org, access is granted and
       the rule is not evaluated at all.
    3. Otherwise each condition is *org-level OR subgroup key*:
       ``"engg.weil"`` passes on the org-level ``weil`` key, else on
       ``engg_<addr>``. An org-only condition (``"weil"``) is just the
       org-level result.

    ``check_key(org, key)`` asks whether *key* holds the required purpose on
    *org*'s identity applet.
    """
    roots: dict[str, Outcome] = {}
    for org in secured_orgs(expr):
        roots[org] = await check_key(org, wallet_addr)

    # Org-level access passes without evaluating the rule.
    if roots and all(root is True for root in roots.values()):
        return True

    async def check(org: str, subgroup: str) -> Outcome:
        root = roots[org] if org in roots else await check_key(org, wallet_addr)
        if not subgroup or root is True:
            return root
        return outcome_or(root, await check_key(org, f"{subgroup}_{wallet_addr}"))

    return await evaluate_secured_expr(expr, check)
