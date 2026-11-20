package weiladk

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

// borrowedTrace is a caller's trace that a method served by
// AgentHTTPHandler records into (RecordToolCall), as Python's agent_http_app
// attaches the trace a request carries. The method never creates, indexes
// or publishes it.
type borrowedTrace struct {
	id      *Identity
	traceID string
	d       *Dispatcher

	mu sync.Mutex
	// parent is the node the next event goes under: the caller's node, then
	// the event recorded last, so events recorded one after another chain.
	parent string
}

type borrowedTraceKey struct{}

func borrowedTraceFrom(ctx context.Context) *borrowedTrace {
	bt, _ := ctx.Value(borrowedTraceKey{}).(*borrowedTrace)
	return bt
}

// borrowTrace returns ctx carrying the trace caller signed, if any, for
// RecordToolCall and the ADK runs started from ctx, recorded as id.
func borrowTrace(ctx context.Context, id *Identity, caller *Caller) (context.Context, *borrowedTrace) {
	if caller == nil || caller.Trace == nil {
		return ctx, nil
	}
	bt := &borrowedTrace{id: id, traceID: caller.Trace.TraceID, d: DefaultDispatcher(), parent: caller.Trace.ParentID}
	return context.WithValue(ctx, borrowedTraceKey{}, bt), bt
}

func (bt *borrowedTrace) context() *client.TraceContext {
	bt.mu.Lock()
	defer bt.mu.Unlock()
	return &client.TraceContext{TraceID: bt.traceID, ParentID: bt.parent}
}

// sync waits until the events queued so far have been submitted, or
// traceSyncTimeout passes.
func (bt *borrowedTrace) sync(ctx context.Context) {
	done := make(chan struct{})
	if err := bt.d.submitOrdered(ctx, "trace:"+bt.traceID, func(context.Context) error {
		close(done)
		return nil
	}); err != nil {
		return
	}
	select {
	case <-done:
	case <-time.After(traceSyncTimeout):
	case <-ctx.Done():
	}
}

// attachedTraceContext is the trace a run started from ctx records into
// when it serves a remote call: the trace its AgentHTTPHandler method
// borrowed (under the event that method recorded last), else the one the
// verified caller signed (SecureA2A), else nil.
func attachedTraceContext(ctx context.Context) *client.TraceContext {
	if bt := borrowedTraceFrom(ctx); bt != nil {
		return bt.context()
	}
	if caller, ok := secured.CallerFromContext(ctx); ok && caller.Trace != nil {
		return caller.Trace
	}
	return nil
}

// RecordToolCall appends one "tool_call" node {"args", "result", "tool"}
// to the trace a method served by AgentHTTPHandler borrowed from its caller
// — Python's weil_ai.tracer.record_tool_call inside agent_http_app. The
// first node goes under the caller's node, each later one under the node
// recorded before it. build_trace is signed by the served agent's identity
// and submitted in the background, in order; the handler replies once the
// calls are submitted. Failures are logged and never fail the method.
//
// Outside such a method it does nothing: an ADK run's tool calls are
// recorded by WithTracing.
func RecordToolCall(ctx context.Context, tool string, args, result map[string]any) {
	bt := borrowedTraceFrom(ctx)
	if bt == nil {
		return
	}
	bt.mu.Lock()
	node, err := toolCallNode(uuid.NewString(), bt.parent, tool, args, result)
	if err == nil {
		bt.parent = node.ID
	}
	bt.mu.Unlock()
	if err != nil {
		slog.Warn("weiladk: trace call failed", "trace_id", bt.traceID, "error", fmt.Errorf("encode tool call: %w", err))
		return
	}
	call := Call{Applet: Tracer, Method: "build_trace", FireAndForget: true, Retry: client.DurableRetry,
		Args: pyArgs(struct {
			TraceID string    `json:"trace_id"`
			Event   TraceNode `json:"event"`
		}{bt.traceID, node})}
	err = bt.d.submitOrdered(ctx, "trace:"+bt.traceID, func(ctx context.Context) error {
		_, err := bt.d.invoke(ctx, bt.id, call)
		if err != nil {
			slog.Warn("weiladk: trace call failed", "trace_id", bt.traceID, "method", "build_trace", "error", err)
		}
		return err
	})
	if err != nil {
		slog.Warn("weiladk: trace call failed", "trace_id", bt.traceID, "method", "build_trace", "error", err)
	}
}
