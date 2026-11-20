package com.weilliptic.weilwallet;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Boolean access rules for secured tools and agents: {@code @WeilSecured},
 * {@code PermissionGate} and the A2A service gate.
 *
 * <p>A rule combines scopes with {@code !} (not), {@code &} (and) and
 * {@code |} (or). {@code !} binds tightest, then {@code &}, then {@code |};
 * {@code &} and {@code |} are left-associative and parentheses override.</p>
 *
 * <pre>
 * rule     :=  and_part ( "|" and_part )*
 * and_part :=  atom     ( "&amp;" atom )*
 * atom     :=  "!" atom  |  "(" rule ")"  |  name
 * </pre>
 *
 * <p>A name is {@code "subgroup.org"}, or {@code "org"} for org-level, bare
 * ({@code engg.weil}) or quoted ({@code "engg.weil"}). The last
 * {@code .}-segment is the org; earlier segments are the subgroup, joined by
 * {@code _}: {@code "a.b.weil"} is subgroup {@code a_b} in org {@code weil}.</p>
 *
 * <p>Evaluation is three-valued: {@code TRUE}, {@code FALSE}, or {@code null}
 * when a check could not be completed. {@code null} never blocks access
 * through an {@code |} alternative that is fine, and {@code !} of
 * {@code null} stays {@code null}, so a broken lookup can never be what
 * grants access. Access needs the whole rule to be {@code TRUE}. The right
 * side of {@code &}/{@code |} is evaluated only when the left side has not
 * already decided the result.</p>
 *
 * <p>Worked examples, rule by rule, are in {@code SecuredRuleExamplesTest}.</p>
 */
public final class SecuredRule {

    private SecuredRule() {}

    /** A parsed rule. */
    public sealed interface Expr permits Atom, Not, And, Or {}

    /** One {@code "subgroup.org"} (or {@code "org"}) condition. */
    public record Atom(String literal, String org, String subgroup) implements Expr {}

    public record Not(Expr inner) implements Expr {}

    public record And(Expr lhs, Expr rhs) implements Expr {}

    public record Or(Expr lhs, Expr rhs) implements Expr {}

    /** Thrown when a rule cannot be parsed. */
    public static final class ParseException extends IllegalArgumentException {
        public ParseException(String message) {
            super(message);
        }
    }

    /** Asks whether the caller satisfies one condition: TRUE, FALSE, or null when unknown. */
    @FunctionalInterface
    public interface Check {
        Boolean check(String org, String subgroup);
    }

    /** Asks whether {@code key} holds the required purpose on {@code org}: TRUE, FALSE, or null. */
    @FunctionalInterface
    public interface KeyCheck {
        Boolean check(String org, String key);
    }

    private static final Map<String, Expr> CACHE = new ConcurrentHashMap<>();

    /**
     * Parse a rule such as {@code "infrastructure.weil & !finance.weil"}.
     *
     * @throws ParseException the rule is empty or malformed
     */
    public static Expr parse(String rule) {
        if (rule == null) {
            throw new ParseException("secured scope must not be empty");
        }
        Expr cached = CACHE.get(rule);
        if (cached != null) {
            return cached;
        }
        Expr expr = new Parser(rule).parse();
        if (CACHE.size() < 256) {
            CACHE.put(rule, expr);
        }
        return expr;
    }

    /**
     * {@code "engg.team.weil"} → {@code ["weil", "engg_team"]}; {@code "weil"} →
     * {@code ["weil", ""]}.
     */
    public static String[] split(String literal) {
        String[] items = literal.split("\\.", -1);
        String org = items[items.length - 1];
        StringBuilder subgroup = new StringBuilder();
        for (int i = 0; i < items.length - 1; i++) {
            if (i > 0) {
                subgroup.append('_');
            }
            subgroup.append(items[i]);
        }
        return new String[] {org, subgroup.toString()};
    }

    /** Every org named anywhere in {@code expr}, sorted. */
    public static List<String> orgs(Expr expr) {
        Set<String> orgs = new TreeSet<>();
        collectOrgs(expr, orgs);
        return new ArrayList<>(orgs);
    }

    private static void collectOrgs(Expr expr, Set<String> orgs) {
        if (expr instanceof Atom a) {
            orgs.add(a.org());
        } else if (expr instanceof Not n) {
            collectOrgs(n.inner(), orgs);
        } else if (expr instanceof And a) {
            collectOrgs(a.lhs(), orgs);
            collectOrgs(a.rhs(), orgs);
        } else if (expr instanceof Or o) {
            collectOrgs(o.lhs(), orgs);
            collectOrgs(o.rhs(), orgs);
        }
    }

    /** Three-valued {@code &}: FALSE wins, then unknown, then TRUE. */
    public static Boolean and(Boolean lhs, Boolean rhs) {
        if (Boolean.FALSE.equals(lhs) || Boolean.FALSE.equals(rhs)) {
            return Boolean.FALSE;
        }
        if (Boolean.TRUE.equals(lhs) && Boolean.TRUE.equals(rhs)) {
            return Boolean.TRUE;
        }
        return null;
    }

    /** Three-valued {@code |}: TRUE wins, then unknown, then FALSE. */
    public static Boolean or(Boolean lhs, Boolean rhs) {
        if (Boolean.TRUE.equals(lhs) || Boolean.TRUE.equals(rhs)) {
            return Boolean.TRUE;
        }
        if (Boolean.FALSE.equals(lhs) && Boolean.FALSE.equals(rhs)) {
            return Boolean.FALSE;
        }
        return null;
    }

    /** Three-valued {@code !}: unknown stays unknown. */
    public static Boolean not(Boolean inner) {
        return inner == null ? null : !inner;
    }

    /**
     * Evaluate {@code expr}, calling {@code check} for each condition. The
     * right side of {@code &}/{@code |} runs only when the left side has not
     * already decided the result.
     */
    public static Boolean evaluate(Expr expr, Check check) {
        if (expr instanceof Atom a) {
            return check.check(a.org(), a.subgroup());
        }
        if (expr instanceof Not n) {
            return not(evaluate(n.inner(), check));
        }
        if (expr instanceof And a) {
            Boolean lhs = evaluate(a.lhs(), check);
            return Boolean.FALSE.equals(lhs) ? Boolean.FALSE : and(lhs, evaluate(a.rhs(), check));
        }
        Or o = (Or) expr;
        Boolean lhs = evaluate(o.lhs(), check);
        return Boolean.TRUE.equals(lhs) ? Boolean.TRUE : or(lhs, evaluate(o.rhs(), check));
    }

    /**
     * Evaluate {@code expr} for {@code walletAddr}, org-level access first.
     *
     * <ol>
     *   <li>For every org the rule names (sorted), the caller's org-level key —
     *       the bare wallet address — is checked once, up front.</li>
     *   <li>If that check is a definite yes for every org, access is granted
     *       and the rule is not evaluated at all.</li>
     *   <li>Otherwise each condition is <em>org-level OR subgroup key</em>:
     *       {@code "engg.weil"} passes on the org-level {@code weil} key, else on
     *       {@code engg_<addr>}. An org-only condition is just the org-level
     *       result.</li>
     * </ol>
     */
    public static Boolean evaluateRootFirst(Expr expr, KeyCheck checkKey, String walletAddr) {
        Map<String, Boolean> roots = new LinkedHashMap<>();
        for (String org : orgs(expr)) {
            roots.put(org, checkKey.check(org, walletAddr));
        }
        // Org-level access passes without evaluating the rule.
        if (!roots.isEmpty() && roots.values().stream().allMatch(Boolean.TRUE::equals)) {
            return Boolean.TRUE;
        }
        return evaluate(expr, (org, subgroup) -> {
            Boolean root = roots.containsKey(org) ? roots.get(org) : checkKey.check(org, walletAddr);
            if (subgroup.isEmpty() || Boolean.TRUE.equals(root)) {
                return root;
            }
            return or(root, checkKey.check(org, subgroup + "_" + walletAddr));
        });
    }

    // ------------------------------------------------------------------ parser

    private record Token(boolean op, String text) {}

    private static final class Parser {
        private final String rule;
        private final List<Token> tokens;
        private int pos;

        Parser(String rule) {
            this.rule = rule;
            this.tokens = tokenize(rule);
        }

        Expr parse() {
            if (tokens.isEmpty()) {
                throw new ParseException("secured scope must not be empty");
            }
            Expr expr = or();
            if (pos < tokens.size()) {
                throw error("unexpected '" + tokens.get(pos).text() + "'");
            }
            return expr;
        }

        private boolean peekOp(String op) {
            return pos < tokens.size() && tokens.get(pos).op() && tokens.get(pos).text().equals(op);
        }

        private Expr or() {
            Expr lhs = and();
            while (peekOp("|")) {
                pos++;
                lhs = new Or(lhs, and());
            }
            return lhs;
        }

        private Expr and() {
            Expr lhs = atom();
            while (peekOp("&")) {
                pos++;
                lhs = new And(lhs, atom());
            }
            return lhs;
        }

        private Expr atom() {
            if (pos >= tokens.size()) {
                throw error("expected a scope, '!' or '(' but the rule ended");
            }
            Token token = tokens.get(pos);
            if (peekOp("!")) {
                pos++;
                return new Not(atom());
            }
            if (peekOp("(")) {
                pos++;
                Expr inner = or();
                if (!peekOp(")")) {
                    throw error("expected ')'");
                }
                pos++;
                return inner;
            }
            if (!token.op()) {
                pos++;
                return atomOf(token.text());
            }
            throw error("expected a scope, '!' or '(' but found '" + token.text() + "'");
        }

        private ParseException error(String message) {
            return new ParseException(message + " in secured rule '" + rule + "'");
        }

        private static Atom atomOf(String literal) {
            String value = literal.trim();
            if (value.isEmpty()) {
                throw new ParseException("secured scope must not be empty");
            }
            for (String part : value.split("\\.", -1)) {
                if (part.isEmpty()) {
                    throw new ParseException("secured scope '" + value
                        + "' must be \"subgroup.org\" or \"org\" for org-level access");
                }
            }
            String[] parts = split(value);
            return new Atom(value, parts[0], parts[1]);
        }

        private static boolean isNameChar(char c) {
            return Character.isLetterOrDigit(c) || c == '_' || c == '-' || c == '.' || c == ':';
        }

        private static List<Token> tokenize(String rule) {
            List<Token> tokens = new ArrayList<>();
            int i = 0;
            while (i < rule.length()) {
                char c = rule.charAt(i);
                if (Character.isWhitespace(c)) {
                    i++;
                    continue;
                }
                if (c == '|' || c == '&' || c == '!') {
                    char next = i + 1 < rule.length() ? rule.charAt(i + 1) : 0;
                    if ((c == '|' && next == '|') || (c == '&' && next == '&') || (c == '!' && next == '=')) {
                        throw new ParseException("unsupported operator '" + c + next + "' in secured rule '"
                            + rule + "' — use single '&', '|' and '!'");
                    }
                    tokens.add(new Token(true, String.valueOf(c)));
                    i++;
                } else if (c == '(' || c == ')') {
                    tokens.add(new Token(true, String.valueOf(c)));
                    i++;
                } else if (c == '"') {
                    int end = rule.indexOf('"', i + 1);
                    if (end < 0) {
                        throw new ParseException("unterminated quote in secured rule '" + rule + "'");
                    }
                    tokens.add(new Token(false, rule.substring(i + 1, end)));
                    i = end + 1;
                } else if (isNameChar(c)) {
                    int start = i;
                    while (i < rule.length() && isNameChar(rule.charAt(i))) {
                        i++;
                    }
                    tokens.add(new Token(false, rule.substring(start, i)));
                } else {
                    throw new ParseException("unexpected character '" + c + "' in secured rule '" + rule + "'");
                }
            }
            return tokens;
        }
    }
}
