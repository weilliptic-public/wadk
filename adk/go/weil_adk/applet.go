package weiladk

import (
	"context"
	"errors"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// SystemicDomain is the domain systemic applets are registered under.
const SystemicDomain = "weil"

// Auditor is the systemic auditor applet ("auditor::weil").
var Auditor = Systemic("auditor")

// Applet identifies an applet either by contract address or by service name.
// The zero value is invalid.
type Applet struct {
	id   string
	name string
}

// AppletID refers to an applet by its contract address.
func AppletID(contractID string) Applet {
	return Applet{id: contractID}
}

// AppletName refers to an applet by its service name, "<applet>::<domain>",
// resolved through Sentinel (and WNS for non-systemic domains).
func AppletName(svcName string) Applet {
	return Applet{name: svcName}
}

// Systemic refers to a systemic applet by its bare name, e.g.
// Systemic("auditor") is "auditor::weil".
func Systemic(name string) Applet {
	return AppletName(name + "::" + SystemicDomain)
}

// ID returns the contract address, or "" for a name reference.
func (a Applet) ID() string { return a.id }

// Name returns the service name, or "" for an address reference.
func (a Applet) Name() string { return a.name }

// IsZero reports whether a names no applet.
func (a Applet) IsZero() bool { return a.id == "" && a.name == "" }

func (a Applet) String() string {
	if a.id != "" {
		return a.id
	}
	return a.name
}

// Resolve returns the applet's contract address. Name lookups are cached per
// account by the client.
func (a Applet) Resolve(ctx context.Context, c *client.WeilClient) (string, error) {
	switch {
	case a.id != "":
		return a.id, nil
	case a.name != "":
		return c.ResolveApplet(ctx, a.name)
	default:
		return "", errors.New("weiladk: call has no applet")
	}
}
