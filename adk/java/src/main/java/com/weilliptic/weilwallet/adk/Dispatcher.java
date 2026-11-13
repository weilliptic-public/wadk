package com.weilliptic.weilwallet.adk;

import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Delivers the calls of {@link Mode#NON_BLOCKING} actions: audit records,
 * stream entries and permission verdicts.
 *
 * <p>Two ways, chosen when it is made:</p>
 *
 * <ul>
 *   <li><b>Inline</b> ({@link #inline()}, the default everywhere in this
 *       package): the call is made on the caller's thread before
 *       {@link #submit} returns, as the Python SDK makes its audit writes. A
 *       record is never left in memory to be dropped by a full queue or lost
 *       when the process exits, and records land in the order they were
 *       made. The cost is one round-trip per record on the agent's own
 *       thread; an audit call is sent fire-and-forget, so that is the
 *       platform accepting it, not the chain executing it.</li>
 *   <li><b>Queued</b> ({@link #Dispatcher(int, int, Overflow, long)}): the
 *       call goes on a background queue and the agent never waits. Mirrors
 *       {@code weiladk.Dispatcher} in the Go SDK. Faster, but calls still
 *       queued when the process stops are lost unless {@link #flush} ran,
 *       and {@link Overflow#DROP} discards calls under load.</li>
 *   <li><b>Ordered</b> ({@link #ordered()}): queued, on one worker, so calls
 *       are sent one at a time in the order they were submitted, and a full
 *       queue makes the caller wait rather than drop anything. What
 *       {@link Weil#plugin} uses: the agent never waits on its records, and
 *       {@link #flush()} at exit sends every one still queued.</li>
 * </ul>
 *
 * <p>Either way a failed call is retried, up to {@link #DEFAULT_ATTEMPTS}
 * attempts in all, before {@code onDone} hears of the failure. A retry after
 * a reply was lost in transit can record an entry twice; a duplicate is
 * visible on chain, where a missing entry is not.</p>
 */
public final class Dispatcher implements AutoCloseable {

    /** What to do when the queue is full. */
    public enum Overflow {
        /** Drop the call and report it. Keeps the agent fast under load. */
        DROP,
        /** Run the call on the caller's thread instead. Keeps every record. */
        WAIT
    }

    /** Receives the outcome of a submitted call. */
    @FunctionalInterface
    public interface Completion {
        void accept(WeilResult result, Throwable error);
    }

    /** Attempts per call, the first included. */
    public static final int DEFAULT_ATTEMPTS = 3;

    /** Wait before the first retry; doubled for each one after. */
    private static final long RETRY_BACKOFF_MILLIS = 500L;

    /** Null when inline. */
    private final ThreadPoolExecutor pool;
    private final Overflow overflow;
    private final long callTimeoutMillis;
    private final int attempts;

    private final AtomicLong submitted = new AtomicLong();
    private final AtomicLong completed = new AtomicLong();
    private final AtomicLong dropped = new AtomicLong();
    private final AtomicLong failed = new AtomicLong();
    private final AtomicLong retried = new AtomicLong();

    /** Calls made on the caller's thread, so none is ever left unsent. */
    public static Dispatcher inline() {
        return new Dispatcher(DEFAULT_ATTEMPTS);
    }

    private Dispatcher(int attempts) {
        this.pool = null;
        this.overflow = Overflow.WAIT;
        this.callTimeoutMillis = 0L;
        this.attempts = attempts;
    }

    /**
     * One worker, so calls go out in the order submitted: a trace is started
     * before its nodes are added. A full queue, 10,000 calls, makes the
     * submitter send its call itself rather than drop it.
     */
    public static Dispatcher ordered() {
        return new Dispatcher(1, 10_000, Overflow.WAIT, 60_000L);
    }

    /** A queue of 256 calls on 2 workers, dropping calls when full. */
    public Dispatcher() {
        this(2, 256, Overflow.DROP, 60_000L);
    }

    /**
     * A background queue.
     *
     * @param workers           concurrent in-flight calls
     * @param queueSize         pending calls held before the overflow policy applies
     * @param overflow          what to do when the queue is full
     * @param callTimeoutMillis per-call timeout; 0 for none
     */
    public Dispatcher(int workers, int queueSize, Overflow overflow, long callTimeoutMillis) {
        this.overflow = overflow;
        this.callTimeoutMillis = callTimeoutMillis;
        this.attempts = DEFAULT_ATTEMPTS;
        ThreadFactory factory = r -> {
            Thread t = new Thread(r, "weil-dispatcher");
            t.setDaemon(true);
            return t;
        };
        this.pool = new ThreadPoolExecutor(
            workers, workers, 0L, TimeUnit.MILLISECONDS,
            new ArrayBlockingQueue<>(queueSize), factory,
            // WAIT runs a rejected call on the caller even after close, where
            // CallerRunsPolicy would silently discard it.
            overflow == Overflow.WAIT
                ? (task, executor) -> task.run()
                : new ThreadPoolExecutor.AbortPolicy());
    }

    /** True when calls are made on the caller's thread. */
    public boolean isInline() {
        return pool == null;
    }

    /**
     * Make {@code call} as {@code identity}: before returning when inline,
     * else queued. {@code onDone} hears the outcome after any retries.
     */
    public void submit(WeilIdentity identity, Call call, Completion onDone) {
        submitted.incrementAndGet();
        // Closed already, at exit: a late call is sent here rather than lost.
        if (pool == null || pool.isShutdown()) {
            deliver(identity, call, onDone);
            return;
        }
        try {
            pool.execute(() -> deliver(identity, call, onDone));
        } catch (RejectedExecutionException e) {
            dropped.incrementAndGet();
            if (onDone != null) {
                onDone.accept(null, e);
            }
        }
    }

    private void deliver(WeilIdentity identity, Call call, Completion onDone) {
        Throwable last = null;
        for (int attempt = 1; attempt <= attempts; attempt++) {
            try {
                WeilResult result = identity.invoke(call);
                completed.incrementAndGet();
                if (onDone != null) {
                    onDone.accept(result, null);
                }
                return;
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                last = e;
                break;
            } catch (Throwable t) {
                last = t;
            }
            if (attempt < attempts) {
                retried.incrementAndGet();
                try {
                    Thread.sleep(RETRY_BACKOFF_MILLIS << (attempt - 1));
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    break;
                }
            }
        }
        failed.incrementAndGet();
        if (onDone != null) {
            onDone.accept(null, last);
        }
    }

    /**
     * Wait up to {@code timeoutMillis} for queued calls to drain.
     *
     * <p>Only matters for a queued dispatcher, whose daemon workers die with
     * the process and take any call still queued with them. Inline, every
     * call was sent before {@link #submit} returned, so nothing is ever
     * pending and this returns true at once.</p>
     *
     * @return true when the queue emptied in time
     */
    public boolean flush(long timeoutMillis) {
        if (pool == null) {
            return true;
        }
        long deadline = System.currentTimeMillis() + timeoutMillis;
        while (System.currentTimeMillis() < deadline) {
            if (pending() == 0) {
                return true;
            }
            try {
                Thread.sleep(50L);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return false;
            }
        }
        return pending() == 0;
    }

    /**
     * Wait, however long it takes, until every queued call has been sent or
     * has failed all its attempts. It cannot hang: each attempt times out
     * with its HTTP request.
     *
     * @return false only when interrupted
     */
    public boolean flush() {
        return flush(Long.MAX_VALUE / 2);
    }

    /**
     * Calls queued or in flight; always 0 inline. Counted from what was
     * submitted and what has finished, so a call between the queue and a
     * worker still counts.
     */
    public long pending() {
        return pool == null ? 0
            : submitted.get() - completed.get() - failed.get() - dropped.get();
    }

    /** What happened to the calls so far, in words: {@code "46 sent, 0 failed, 1 retried"}. */
    public String summary() {
        String text = completed.get() + " sent, " + failed.get() + " failed";
        if (retried.get() > 0) {
            text += ", " + retried.get() + " retried";
        }
        if (dropped.get() > 0) {
            text += ", " + dropped.get() + " dropped";
        }
        return text;
    }

    long callTimeoutMillis() {
        return callTimeoutMillis;
    }

    public Overflow overflowPolicy() {
        return overflow;
    }

    /** Counters, for logging and tests. */
    public String stats() {
        return "submitted=" + submitted.get()
            + " completed=" + completed.get()
            + " failed=" + failed.get()
            + " retried=" + retried.get()
            + " dropped=" + dropped.get()
            + " pending=" + (pool == null ? 0 : pool.getQueue().size());
    }

    /**
     * Stop the worker threads; call {@link #flush} first, or queued calls are
     * dropped. Only matters for a queued dispatcher: inline, there are no
     * threads to stop.
     */
    @Override
    public void close() {
        if (pool != null) {
            pool.shutdown();
        }
    }
}
