package client

import (
	"crypto/tls"
	"net/http"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/internal/constants"
)

// DefaultTimeout bounds each HTTP exchange with Sentinel when no custom
// http.Client is supplied, as the Python SDK's 60 s. Blocking executions wait
// for the applet to run; use a context deadline for tighter bounds.
const DefaultTimeout = 60 * time.Second

// DefaultConcurrency is how many transactions a WeilClient submits at once,
// as the Python SDK's DEFAULT_CONCURRENCY; more wait their turn.
const DefaultConcurrency = 64

// S3Credentials point Sentinel at a caller-owned bucket holding agent wallets.
type S3Credentials struct {
	AccessKeyID     string `json:"access_key_id"`
	SecretAccessKey string `json:"secret_access_key"`
	BucketName      string `json:"bucket_name"`
	Region          string `json:"region"`
}

// Option configures a WeilClient.
type Option func(*config)

type config struct {
	sentinelHost       string
	httpClient         *http.Client
	timeout            time.Duration
	insecureSkipVerify bool
	creds              *S3Credentials
	signer             Signer
	concurrency        int
}

func newConfig(opts []Option) *config {
	cfg := &config{
		sentinelHost: constants.SENTINEL_HOST,
		timeout:      DefaultTimeout,
		concurrency:  DefaultConcurrency,
	}
	for _, opt := range opts {
		opt(cfg)
	}
	if cfg.httpClient == nil {
		transport := http.DefaultTransport.(*http.Transport).Clone()
		if cfg.insecureSkipVerify {
			transport.TLSClientConfig = &tls.Config{InsecureSkipVerify: true}
		}
		cfg.httpClient = &http.Client{Transport: transport, Timeout: cfg.timeout}
	}
	return cfg
}

// WithSentinelHost points the client at a different Sentinel, e.g. staging.
func WithSentinelHost(host string) Option {
	return func(c *config) { c.sentinelHost = host }
}

// WithHTTPClient supplies the http.Client used for every Sentinel call. It
// takes precedence over WithTimeout and WithInsecureSkipVerify.
func WithHTTPClient(httpClient *http.Client) Option {
	return func(c *config) { c.httpClient = httpClient }
}

// WithTimeout sets the per-request timeout of the default http.Client.
func WithTimeout(timeout time.Duration) Option {
	return func(c *config) { c.timeout = timeout }
}

// WithConcurrency bounds how many transactions the client submits at once
// (default DefaultConcurrency); n <= 0 removes the bound.
func WithConcurrency(n int) Option {
	return func(c *config) { c.concurrency = n }
}

// WithInsecureSkipVerify disables TLS certificate verification. Only for
// development against Sentinels with self-signed certificates.
func WithInsecureSkipVerify() Option {
	return func(c *config) { c.insecureSkipVerify = true }
}

// WithS3Credentials supplies credentials for a caller-owned wallet bucket.
// They are sent to Sentinel when resolving and remote-signing for API-key
// wallets.
func WithS3Credentials(creds *S3Credentials) Option {
	return func(c *config) { c.creds = creds }
}

// WithSigner replaces the built-in signer selection (local when the account
// holds a secret key, Sentinel remote signing for masked wallets) with a
// custom Signer, e.g. one backed by a KMS or HSM.
func WithSigner(signer Signer) Option {
	return func(c *config) { c.signer = signer }
}
