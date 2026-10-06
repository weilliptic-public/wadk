"""Customer-support database — only what the example tests.

Seed story (matches test_tickets.json):
  TKT-01  ORD-12345 ($120) captured twice -> $120 verified duplicate.
  TKT-06  ORD-20003 is a real $30 late fee.
  TKT-07  ORD-98765 ($200) is PRO-009's last payment, as the ticket says.
  TKT-09  FRE-009 has 3 refunds in 14 days -> velocity flagged.
  TKT-12  ORD-70001 ($400) captured twice -> $400 verified duplicate.
"""
from __future__ import annotations

import sqlite3
from datetime import datetime, timedelta, timezone


def init_db() -> sqlite3.Connection:
    conn = sqlite3.connect(":memory:", check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys=ON")
    _create_schema(conn)
    _seed(conn)
    return conn


def _create_schema(conn: sqlite3.Connection) -> None:
    conn.executescript("""
        CREATE TABLE customers (
            customer_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            tier TEXT NOT NULL CHECK (tier IN ('free', 'pro', 'enterprise')),
            account_status TEXT NOT NULL DEFAULT 'active',
            lifetime_value REAL NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE orders (
            order_id TEXT PRIMARY KEY,
            customer_id TEXT NOT NULL REFERENCES customers(customer_id),
            status TEXT NOT NULL
                CHECK (status IN ('pending', 'paid', 'failed', 'cancelled')),
            total REAL NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL
        );

        -- One row per charge attempt: a double-charge is two SUCCEEDED rows
        -- for the same order_id (Stripe-style), never a duplicated order PK.
        CREATE TABLE payments (
            payment_id TEXT PRIMARY KEY,
            order_id TEXT NOT NULL REFERENCES orders(order_id),
            amount REAL NOT NULL,
            status TEXT NOT NULL
                CHECK (status IN ('succeeded', 'failed', 'refunded', 'partially_refunded')),
            created_at TEXT NOT NULL
        );

        CREATE TABLE refunds (
            refund_id TEXT PRIMARY KEY,
            payment_id TEXT NOT NULL REFERENCES payments(payment_id),
            order_id TEXT NOT NULL REFERENCES orders(order_id),
            customer_id TEXT NOT NULL REFERENCES customers(customer_id),
            amount REAL NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'succeeded', 'failed')),
            created_at TEXT NOT NULL
        );

        CREATE TABLE tickets (
            ticket_id TEXT PRIMARY KEY,
            customer_id TEXT NOT NULL REFERENCES customers(customer_id),
            channel TEXT NOT NULL DEFAULT 'email',
            subject TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            priority TEXT NOT NULL DEFAULT 'medium',
            assignee_team TEXT,
            created_at TEXT NOT NULL
        );

        -- No FK on ticket_id: actions log for every processed ticket, even
        -- ones never stored as rows (e.g. adversarial probes).
        CREATE TABLE interactions (
            interaction_id INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_id TEXT,
            agent TEXT NOT NULL,
            action TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX idx_orders_customer ON orders(customer_id);
        CREATE INDEX idx_payments_order ON payments(order_id);
        CREATE INDEX idx_refunds_customer_created ON refunds(customer_id, created_at);
        CREATE INDEX idx_tickets_customer_status ON tickets(customer_id, status);
        CREATE INDEX idx_interactions_ticket ON interactions(ticket_id);
    """)


def _seed(conn: sqlite3.Connection) -> None:
    now = datetime.now(timezone.utc)

    def ts(**kw) -> str:
        return (now - timedelta(**kw)).isoformat()

    conn.executemany(
        "INSERT INTO customers (customer_id, name, email, tier,"
        " account_status, lifetime_value, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        [("FRE-002", "Alice Chen", "alice@example.com", "free", "active", 120.0, ts(days=60)),
         ("FRE-003", "Bob Smith", "bob@example.com", "free", "active", 0.0, ts(days=10)),
         ("FRE-007", "Carol Davis", "carol@example.com", "free", "active", 0.0, ts(days=5)),
         ("FRE-009", "Eve Martinez", "eve@example.com", "free", "active", 0.0, ts(days=3)),
         ("FRE-010", "Frank Lee", "frank@example.com", "free", "active", 0.0, ts(days=2)),
         ("FRE-011", "Grace Kim", "grace@example.com", "free", "active", 0.0, ts(days=1)),
         ("PRO-004", "Hank Wilson", "hank@company.com", "pro", "active", 2400.0, ts(days=180)),
         ("PRO-009", "Ivy Brown", "ivy@startup.io", "pro", "active", 800.0, ts(days=90)),
         ("PRO-011", "Jack Taylor", "jack@dev.co", "pro", "active", 1600.0, ts(days=120)),
         ("PRO-020", "Kate Moore", "kate@bigdata.com", "pro", "active", 3200.0, ts(days=30)),
         ("ENT-001", "Leo Zhang", "leo@megacorp.com", "enterprise", "active", 50000.0, ts(days=365)),
         ("ENT-003", "Mia Johnson", "mia@global.io", "enterprise", "active", 35000.0, ts(days=200))])

    conn.executemany(
        "INSERT INTO orders (order_id, customer_id, status, total, description, created_at)"
        " VALUES (?, ?, ?, ?, ?, ?)",
        [("ORD-12345", "FRE-002", "paid", 120.0, "Widget Pro license", ts(days=14)),
         ("ORD-20001", "PRO-004", "paid", 99.0, "Monthly subscription", ts(days=30)),
         ("ORD-20003", "PRO-004", "paid", 30.0, "Late fee - overdue payment", ts(days=35)),
         ("ORD-30001", "ENT-001", "paid", 5000.0, "Enterprise annual license", ts(days=300)),
         ("ORD-30002", "ENT-001", "pending", 1200.0, "Integration setup fee", ts(days=2)),
         ("ORD-30003", "ENT-001", "pending", 800.0, "API access upgrade", ts(days=1)),
         ("ORD-40001", "PRO-011", "paid", 250.0, "Dashboard access", ts(days=60)),
         ("ORD-70001", "PRO-020", "paid", 400.0, "Q3 usage overage", ts(days=20)),
         ("ORD-80001", "FRE-009", "paid", 150.0, "Mini plan monthly", ts(days=20)),
         ("ORD-98765", "PRO-009", "paid", 200.0, "API credits", ts(days=25))])

    conn.executemany(
        "INSERT INTO payments (payment_id, order_id, amount, status, created_at)"
        " VALUES (?, ?, ?, ?, ?)",
        [# ORD-12345 captured twice at $120; the failed retry was never charged.
         ("PAY-12345-A", "ORD-12345", 120.0, "succeeded", ts(days=14)),
         ("PAY-12345-B", "ORD-12345", 120.0, "succeeded", ts(days=13)),
         ("PAY-12345-C", "ORD-12345", 120.0, "failed", ts(days=13)),
         ("PAY-20001-A", "ORD-20001", 99.0, "succeeded", ts(days=30)),
         ("PAY-20003-A", "ORD-20003", 30.0, "succeeded", ts(days=35)),
         ("PAY-30001-A", "ORD-30001", 5000.0, "succeeded", ts(days=300)),
         ("PAY-40001-A", "ORD-40001", 250.0, "succeeded", ts(days=60)),
         ("PAY-70001-A", "ORD-70001", 400.0, "partially_refunded", ts(days=20)),
         ("PAY-70001-B", "ORD-70001", 400.0, "succeeded", ts(days=19)),
         ("PAY-80001-A", "ORD-80001", 50.0, "refunded", ts(days=20)),
         ("PAY-80001-B", "ORD-80001", 75.0, "refunded", ts(days=15)),
         ("PAY-80001-C", "ORD-80001", 100.0, "refunded", ts(days=10)),
         ("PAY-98765-A", "ORD-98765", 200.0, "succeeded", ts(days=25))])

    conn.executemany(
        "INSERT INTO refunds (refund_id, payment_id, order_id, customer_id, amount, status, created_at)"
        " VALUES (?, ?, ?, ?, ?, ?, ?)",
        [# FRE-009: 3 refunds in 14 days -> velocity flagged.
         ("REF-001", "PAY-80001-A", "ORD-80001", "FRE-009", 50.0, "succeeded", ts(days=12)),
         ("REF-002", "PAY-80001-B", "ORD-80001", "FRE-009", 75.0, "succeeded", ts(days=7)),
         ("REF-003", "PAY-80001-C", "ORD-80001", "FRE-009", 100.0, "succeeded", ts(days=3)),
         ("REF-004", "PAY-70001-A", "ORD-70001", "PRO-020", 40.0, "succeeded", ts(days=10))])

    conn.executemany(
        "INSERT INTO tickets (ticket_id, customer_id, channel, subject, status, priority,"
        " assignee_team, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        [("TKT-01", "FRE-002", "email", "Charged twice for order #12345",
          "open", "high", "billing_team", ts(hours=2)),
         ("TKT-02", "FRE-003", "chat", "How do I reset my password?",
          "open", "low", "tech_support", ts(hours=1)),
         ("TKT-05", "ENT-001", "phone", "URGENT: entire team locked out!!",
          "open", "critical", "account_management", ts(minutes=30)),
         ("TKT-08", "ENT-003", "chat", "Cannot integrate Salesforce, 401 error",
          "open", "high", "tech_support", ts(hours=4)),
         ("HIST-001", "PRO-004", "email", "Previous refund request",
          "resolved", "medium", "billing_team", ts(days=30)),
         ("HIST-002", "FRE-002", "chat", "API issue",
          "resolved", "medium", "tech_support", ts(days=7))])

    conn.commit()


# ------------------------------------------------------------------ reads

def get_customer(conn: sqlite3.Connection, customer_id: str) -> dict | None:
    row = conn.execute("SELECT * FROM customers WHERE customer_id = ?", (customer_id,)).fetchone()
    return dict(row) if row else None


def get_orders(conn: sqlite3.Connection, customer_id: str) -> list[dict]:
    rows = conn.execute(
        "SELECT * FROM orders WHERE customer_id = ? ORDER BY created_at DESC", (customer_id,)).fetchall()
    return [dict(r) for r in rows]


def get_order(conn: sqlite3.Connection, order_id: str) -> dict | None:
    row = conn.execute("SELECT * FROM orders WHERE order_id = ?", (order_id,)).fetchone()
    return dict(row) if row else None


def get_payments_for_order(conn: sqlite3.Connection, order_id: str) -> list[dict]:
    rows = conn.execute(
        "SELECT * FROM payments WHERE order_id = ? ORDER BY created_at", (order_id,)).fetchall()
    return [dict(r) for r in rows]


def find_duplicate_charges(conn: sqlite3.Connection, order_id: str) -> list[dict]:
    """Succeeded captures beyond the first = money taken twice."""
    rows = conn.execute(
        "SELECT * FROM payments WHERE order_id = ? AND status IN ('succeeded', 'partially_refunded')"
        " ORDER BY created_at", (order_id,)).fetchall()
    captured = [dict(r) for r in rows]
    return captured[1:] if len(captured) > 1 else []


def order_payment_summary(conn: sqlite3.Connection, customer_id: str,
                          limit: int = 8) -> list[dict]:
    """Per-order capture snapshot for LLM evidence."""
    rows = conn.execute(
        "SELECT o.order_id, o.total, o.status, o.description,"
        " COUNT(CASE WHEN p.status IN ('succeeded', 'partially_refunded', 'refunded')"
        " THEN 1 END) AS captures,"
        " COALESCE(SUM(CASE WHEN p.status IN ('succeeded', 'partially_refunded', 'refunded')"
        " THEN p.amount END), 0) AS captured"
        " FROM orders o LEFT JOIN payments p ON p.order_id = o.order_id"
        " WHERE o.customer_id = ? GROUP BY o.order_id ORDER BY o.created_at DESC LIMIT ?",
        (customer_id, max(1, limit))).fetchall()
    return [dict(r) for r in rows]


def get_tickets_for_customer(conn: sqlite3.Connection, customer_id: str,
                             limit: int = 5) -> list[dict]:
    rows = conn.execute(
        "SELECT ticket_id, subject, status, priority, created_at FROM tickets"
        " WHERE customer_id = ? ORDER BY created_at DESC LIMIT ?",
        (customer_id, max(1, limit))).fetchall()
    return [dict(r) for r in rows]


def customer_captured_total(conn: sqlite3.Connection, customer_id: str) -> float:
    """Lifetime captured across all of a customer's payments (grounding check)."""
    row = conn.execute(
        "SELECT COALESCE(SUM(p.amount), 0) FROM payments p"
        " JOIN orders o ON o.order_id = p.order_id"
        " WHERE o.customer_id = ? AND p.status IN ('succeeded', 'partially_refunded', 'refunded')",
        (customer_id,)).fetchone()
    return float(row[0]) if row else 0.0


def get_refunds(conn: sqlite3.Connection, customer_id: str, days: int = 30) -> list[dict]:
    cutoff = (datetime.now(timezone.utc) - timedelta(days=days)).isoformat()
    rows = conn.execute(
        "SELECT * FROM refunds WHERE customer_id = ? AND created_at >= ? ORDER BY created_at DESC",
        (customer_id, cutoff)).fetchall()
    return [dict(r) for r in rows]


def log_interaction(conn: sqlite3.Connection, ticket_id: str, agent: str, action: str) -> None:
    conn.execute(
        "INSERT INTO interactions (ticket_id, agent, action, created_at) VALUES (?, ?, ?, ?)",
        (ticket_id, agent, action, datetime.now(timezone.utc).isoformat()))
    conn.commit()
