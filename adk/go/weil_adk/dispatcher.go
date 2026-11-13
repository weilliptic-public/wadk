package weiladk

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// OverflowPolicy decides what Submit does when the dispatcher queue is full.
type OverflowPolicy int

const (
	// OverflowBlock waits for room (or for the submitting context to end).
	// Nothing is lost, at the cost of back-pressure on the agent.
	OverflowBlock OverflowPolicy = iota
	// OverflowDrop discards the job and returns ErrQueueFull.
	OverflowDrop
)

var (
	// ErrQueueFull is returned by Submit under OverflowDrop when the queue is full.
	ErrQueueFull = errors.New("weiladk: dispatcher queue full")
	// ErrDispatcherClosed is returned by Submit after Close.
	ErrDispatcherClosed = errors.New("weiladk: dispatcher closed")
)

// RetryPolicy retries failed calls with exponential backoff. Applet-level
// errors (*transaction.AppletError) are never retried.
//
// Retrying is not idempotent: a call that reached the chain before the error
// surfaced (e.g. a timeout waiting for the result) runs again, under a new
// transaction. Only enable retries for methods that tolerate duplicates.
type RetryPolicy struct {
	// MaxAttempts counts the first try; 0 or 1 disables retries.
	MaxAttempts    int
	InitialBackoff time.Duration
	MaxBackoff     time.Duration
}

// DispatcherConfig configures a Dispatcher. Zero values pick the defaults.
type DispatcherConfig struct {
	// Workers is the number of calls in flight at once. Default 4.
	Workers int
	// DurableWorkers run calls with their own retry policy (Call.Retry),
	// such as audit entries, apart from Workers, so slow blocking calls
	// queued ahead of them cannot hold them up. Default 64, the client's
	// default submission concurrency.
	DurableWorkers int
	// QueueSize bounds the jobs waiting for a worker. Default 1024.
	QueueSize int
	Overflow  OverflowPolicy
	Retry     RetryPolicy
	// CallTimeout bounds each attempt. Default 30s.
	CallTimeout time.Duration
	// OnResult, if set, observes every finished job, after the job's own
	// callback.
	OnResult func(call Call, res *Result, err error)
}

// DispatcherStats counts jobs over the dispatcher's lifetime.
type DispatcherStats struct {
	Submitted, Succeeded, Failed, Dropped uint64
}

// Dispatcher runs applet calls in the background so hooks never wait on the
// chain. It is safe for concurrent use.
type Dispatcher struct {
	cfg   DispatcherConfig
	queue chan job
	// durable carries calls with their own retry policy (see DurableWorkers).
	durable chan job

	mu      sync.RWMutex // guards closed against concurrent Submit
	closed  bool
	pending sync.WaitGroup
	workers sync.WaitGroup

	laneMu sync.Mutex
	lanes  map[string][]job // jobs waiting their turn, per ordered lane

	submitted, succeeded, failed, dropped atomic.Uint64
}

type job struct {
	ctx    context.Context
	id     *Identity
	call   Call
	onDone func(*Result, error)

	// fn, if set, runs instead of call (see submitOrdered).
	fn func(context.Context) error
	// lane, if set, marks a job that drains that lane rather than a call.
	lane string
}

// NewDispatcher starts a dispatcher's workers.
func NewDispatcher(cfg DispatcherConfig) *Dispatcher {
	if cfg.Workers <= 0 {
		cfg.Workers = 4
	}
	if cfg.DurableWorkers <= 0 {
		cfg.DurableWorkers = 64
	}
	if cfg.QueueSize <= 0 {
		cfg.QueueSize = 1024
	}
	if cfg.CallTimeout <= 0 {
		cfg.CallTimeout = 30 * time.Second
	}
	if cfg.Retry.InitialBackoff <= 0 {
		cfg.Retry.InitialBackoff = 200 * time.Millisecond
	}
	if cfg.Retry.MaxBackoff <= 0 {
		cfg.Retry.MaxBackoff = 5 * time.Second
	}

	d := &Dispatcher{cfg: cfg, queue: make(chan job, cfg.QueueSize), durable: make(chan job, cfg.QueueSize), lanes: map[string][]job{}}
	d.workers.Add(cfg.Workers + cfg.DurableWorkers)
	for range cfg.Workers {
		go d.work(d.queue)
	}
	for range cfg.DurableWorkers {
		go d.work(d.durable)
	}
	return d
}

// Submit queues call to run as id. onDone, if non-nil, runs on a worker when
// the call finishes.
//
// The job outlives ctx's cancellation (hooks' contexts end with the
// invocation) but keeps its values; each attempt is bounded by CallTimeout.
// Under OverflowBlock, ctx does bound how long Submit waits for queue room.
//
// A call with its own retry policy (Call.Retry, e.g. an audit entry) is
// never dropped: whatever Overflow says and whether or not ctx ends, Submit
// waits for room in the durable queue, so a congested platform slows the
// caller down rather than losing the call.
func (d *Dispatcher) Submit(ctx context.Context, id *Identity, call Call, onDone func(*Result, error)) error {
	d.mu.RLock()
	defer d.mu.RUnlock()
	if d.closed {
		return ErrDispatcherClosed
	}

	j := job{ctx: context.WithoutCancel(ctx), id: id, call: call, onDone: onDone}
	d.pending.Add(1)

	if call.Retry.MaxAttempts > 1 {
		d.durable <- j
		d.submitted.Add(1)
		return nil
	}
	queue := d.queue
	switch d.cfg.Overflow {
	case OverflowDrop:
		select {
		case queue <- j:
		default:
			d.pending.Done()
			d.dropped.Add(1)
			return ErrQueueFull
		}
	default:
		select {
		case queue <- j:
		case <-ctx.Done():
			d.pending.Done()
			d.dropped.Add(1)
			return ctx.Err()
		}
	}
	d.submitted.Add(1)
	return nil
}

// submitOrdered queues fn on lane: jobs on one lane run one at a time, in
// the order submitted, while other lanes and plain calls proceed alongside.
// fn should make its calls through invoke, so they get the dispatcher's
// timeout and retries. Flush waits for ordered jobs too.
func (d *Dispatcher) submitOrdered(ctx context.Context, lane string, fn func(context.Context) error) error {
	d.mu.RLock()
	defer d.mu.RUnlock()
	if d.closed {
		return ErrDispatcherClosed
	}

	j := job{ctx: context.WithoutCancel(ctx), fn: fn}
	d.pending.Add(1)
	d.laneMu.Lock()
	waiting, active := d.lanes[lane]
	d.lanes[lane] = append(waiting, j)
	d.laneMu.Unlock()
	if active {
		// The lane's drain job is queued or running and will reach this one.
		d.submitted.Add(1)
		return nil
	}

	drain := job{lane: lane}
	var err error
	switch d.cfg.Overflow {
	case OverflowDrop:
		select {
		case d.queue <- drain:
		default:
			err = ErrQueueFull
		}
	default:
		select {
		case d.queue <- drain:
		case <-ctx.Done():
			err = ctx.Err()
		}
	}
	if err != nil {
		// Nothing can have joined the lane: it was not active, and joining
		// one that is not active starts a drain of its own.
		d.laneMu.Lock()
		delete(d.lanes, lane)
		d.laneMu.Unlock()
		d.pending.Done()
		d.dropped.Add(1)
		return err
	}
	d.submitted.Add(1)
	return nil
}

func (d *Dispatcher) work(queue chan job) {
	defer d.workers.Done()
	for j := range queue {
		if j.lane != "" {
			d.drain(j.lane)
			continue
		}
		d.finish(j)
	}
}

// drain runs a lane's jobs until none are waiting, then retires the lane.
func (d *Dispatcher) drain(lane string) {
	for {
		d.laneMu.Lock()
		waiting := d.lanes[lane]
		if len(waiting) == 0 {
			delete(d.lanes, lane)
			d.laneMu.Unlock()
			return
		}
		j := waiting[0]
		// The finished job stays at the head while it runs, so a submit in
		// the meantime sees the lane active and does not start a second drain.
		d.laneMu.Unlock()
		d.finish(j)
		d.laneMu.Lock()
		d.lanes[lane] = d.lanes[lane][1:]
		d.laneMu.Unlock()
	}
}

func (d *Dispatcher) finish(j job) {
	var res *Result
	var err error
	if j.fn != nil {
		err = j.fn(j.ctx)
	} else {
		res, err = d.invoke(j.ctx, j.id, j.call)
	}
	if err != nil {
		d.failed.Add(1)
	} else {
		d.succeeded.Add(1)
	}
	if j.onDone != nil {
		j.onDone(res, err)
	}
	if d.cfg.OnResult != nil && j.fn == nil {
		d.cfg.OnResult(j.call, res, err)
	}
	d.pending.Done()
}

// invoke runs call as id with the dispatcher's timeout and retry policy.
func (d *Dispatcher) invoke(ctx context.Context, id *Identity, call Call) (*Result, error) {
	attempts := max(d.cfg.Retry.MaxAttempts, 1)
	if call.Retry.MaxAttempts > 1 {
		// The call resends its own signed transaction; retrying here would
		// sign it again, as a second transaction.
		attempts = 1
	}
	backoff := d.cfg.Retry.InitialBackoff

	var res *Result
	var err error
	for attempt := 1; ; attempt++ {
		// A call with its own retry policy resends the same signed
		// transaction itself, bounding each attempt and the whole by the
		// transaction's TTL; CallTimeout would cut that short.
		callCtx, cancel := ctx, context.CancelFunc(func() {})
		if call.Retry.MaxAttempts <= 1 {
			callCtx, cancel = context.WithTimeout(ctx, d.cfg.CallTimeout)
		}
		res, err = id.Invoke(callCtx, call)
		cancel()

		var appletErr *transaction.AppletError
		if err == nil || errors.As(err, &appletErr) || attempt >= attempts {
			return res, err
		}
		time.Sleep(backoff)
		backoff = min(backoff*2, d.cfg.Retry.MaxBackoff)
	}
}

// Flush waits until every job submitted so far has finished, or ctx ends.
func (d *Dispatcher) Flush(ctx context.Context) error {
	done := make(chan struct{})
	go func() {
		d.pending.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// Close stops accepting jobs and waits for queued ones to finish, or for ctx
// to end. Workers keep draining the queue in the background if ctx ends first.
func (d *Dispatcher) Close(ctx context.Context) error {
	d.mu.Lock()
	if !d.closed {
		d.closed = true
		close(d.queue)
		close(d.durable)
	}
	d.mu.Unlock()

	done := make(chan struct{})
	go func() {
		d.workers.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// Stats returns lifetime job counts.
func (d *Dispatcher) Stats() DispatcherStats {
	return DispatcherStats{
		Submitted: d.submitted.Load(),
		Succeeded: d.succeeded.Load(),
		Failed:    d.failed.Load(),
		Dropped:   d.dropped.Load(),
	}
}
