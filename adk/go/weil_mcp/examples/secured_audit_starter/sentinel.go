package main

import (
	"net"
	"net/url"
	"os"
	"strings"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// defaultSentinelHost is the Sentinel used when WEIL_SENTINEL_HOST (or
// SENTINEL_HOST) is not set
const defaultSentinelHost = "" //"https://sentinel.weilliptic.ai"

// sentinelOptions points a client at WEIL_SENTINEL_HOST (then SENTINEL_HOST,
// then defaultSentinelHost). Its TLS certificate is verified, except for a
// local platform's self-signed one (an IP address, localhost or
// sentinel-local...) or with WEIL_TLS_INSECURE=1; WEIL_TLS_VERIFY=1 verifies
// a local one too.
func sentinelOptions() []client.Option {
	host := os.Getenv("WEIL_SENTINEL_HOST")
	if host == "" {
		host = os.Getenv("SENTINEL_HOST")
	}
	if host == "" {
		host = defaultSentinelHost
	}
	opts := []client.Option{client.WithSentinelHost(host)}
	if !verifyTLS(host) {
		opts = append(opts, client.WithInsecureSkipVerify())
	}
	return opts
}

func verifyTLS(host string) bool {
	switch {
	case os.Getenv("WEIL_TLS_VERIFY") == "1":
		return true
	case os.Getenv("WEIL_TLS_INSECURE") == "1":
		return false
	}
	h := host
	if u, err := url.Parse(host); err == nil && u.Host != "" {
		h = u.Hostname()
	}
	return !(net.ParseIP(h) != nil || h == "localhost" || strings.HasPrefix(h, "sentinel-local"))
}
