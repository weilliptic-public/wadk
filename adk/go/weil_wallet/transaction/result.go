package transaction

import (
	"encoding/json"
	"errors"
	"fmt"
)

// Transaction lifecycle states, as reported in TransactionResult.Status.
const (
	StatusInProgress = "InProgress"
	StatusConfirmed  = "Confirmed"
	StatusFinalized  = "Finalized"
	StatusFailed     = "Failed"
)

// AppletError is the "Err" side of the {"Ok": ...}/{"Err": ...} envelope the
// platform wraps every applet method's return value in.
type AppletError struct {
	// Message is the applet's error value, as JSON text when it was not a
	// plain string.
	Message string
}

func (e *AppletError) Error() string {
	return "applet returned an error: " + e.Message
}

// ErrNoResult is returned by Decode for a result that carries no applet
// output — notably the empty result of a non-blocking submission.
var ErrNoResult = errors.New("transaction result carries no applet output")

// Failed reports whether the platform marked the transaction as failed.
func (r *TransactionResult) Failed() bool {
	return r.Status == StatusFailed
}

// Decode unwraps the applet's return value from TxnResult into out.
//
// The value is double-wrapped: the platform puts every method's result in an
// {"Ok": ...}/{"Err": ...} envelope, and "Ok"'s value is usually the callee's
// return value re-serialized to a JSON string rather than embedded directly.
// Decode handles both shapes, and returns an *AppletError for "Err".
//
// out may be nil to only check for an applet error.
func (r *TransactionResult) Decode(out any) error {
	if r.TxnResult == "" {
		return ErrNoResult
	}
	return DecodeAppletResult(r.TxnResult, out)
}

// DecodeAppletResult is Decode for a raw txn_result string.
func DecodeAppletResult(txnResult string, out any) error {
	var envelope map[string]json.RawMessage
	value := json.RawMessage(txnResult)
	if err := json.Unmarshal([]byte(txnResult), &envelope); err == nil {
		if errValue, ok := envelope["Err"]; ok {
			return &AppletError{Message: rawToMessage(errValue)}
		}
		if okValue, ok := envelope["Ok"]; ok {
			value = okValue
		}
	}

	if out == nil {
		return nil
	}

	// Ok carries a JSON string: it is usually the return value re-serialized,
	// so decode the inner text first. A plain string return that is not
	// itself JSON falls through to decoding the string as-is.
	var inner string
	if err := json.Unmarshal(value, &inner); err == nil {
		if err := json.Unmarshal([]byte(inner), out); err == nil {
			return nil
		}
	}

	if err := json.Unmarshal(value, out); err != nil {
		return fmt.Errorf("decode applet result: %w", err)
	}
	return nil
}

func rawToMessage(raw json.RawMessage) string {
	var s string
	if err := json.Unmarshal(raw, &s); err == nil {
		return s
	}
	return string(raw)
}
