package http

import (
	"encoding/json"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	internal "github.com/weilliptic-public/wadk/adk/go/weil_go/internal"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

//go:wasmimport env make_http_outcall
func make_http_outcall(ptr int32) int32

// HttpMethod represents the HTTP method to use for a request.
type HttpMethod string

const (
	GET    HttpMethod = "GET"
	POST   HttpMethod = "POST"
	PUT    HttpMethod = "PUT"
	DELETE HttpMethod = "DELETE"
	PATCH  HttpMethod = "PATCH"
	HEAD   HttpMethod = "HEAD"
)

// OutcallRequest matches the Rust struct for HTTP outcalls.
type OutcallRequest struct {
	Url         string            `json:"url"`
	Method      string            `json:"method"`
	Headers     map[string]string `json:"headers"`
	Body        *string           `json:"body"`
	QueryParams [][2]string       `json:"query_params"`
}

// HttpClient is a simple HTTP client for making outcalls.
type HttpClient struct{}

func NewHttpClient() *HttpClient {
	return &HttpClient{}
}

type RequestBuilder struct {
	url         string
	method      HttpMethod
	headers     map[string]string
	body        *string
	queryParams [][2]string
}

func (c *HttpClient) Request(method HttpMethod, url string) *RequestBuilder {
	return &RequestBuilder{
		url:         url,
		method:      method,
		headers:     make(map[string]string),
		queryParams: make([][2]string, 0),
	}
}

func (rb *RequestBuilder) Headers(headers map[string]string) *RequestBuilder {
	for k, v := range headers {
		rb.headers[k] = v
	}
	return rb
}

func (rb *RequestBuilder) Query(params map[string]string) *RequestBuilder {
	for k, v := range params {
		rb.queryParams = append(rb.queryParams, [2]string{k, v})
	}
	return rb
}

func (rb *RequestBuilder) Body(body string) *RequestBuilder {
	rb.body = &body
	return rb
}

func (rb *RequestBuilder) JSON(data any) *RequestBuilder {
	b, err := json.Marshal(data)
	if err == nil {
		s := string(b)
		rb.body = &s
		rb.headers["Content-Type"] = "application/json"
	}
	return rb
}

func (rb *RequestBuilder) Form(data map[string]string) *RequestBuilder {
	form := ""
	first := true
	for k, v := range data {
		if !first {
			form += "&"
		}
		form += urlEncode(k) + "=" + urlEncode(v)
		first = false
	}
	rb.body = &form
	rb.headers["Content-Type"] = "application/x-www-form-urlencoded"
	return rb
}

func urlEncode(input string) string {
	// RFC 3986
	out := ""
	for _, c := range input {
		if (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.' || c == '~' {
			out += string(c)
		} else if c == ' ' {
			out += "+"
		} else {
			buf := make([]byte, 4)
			n := utf8EncodeRune(buf, c)
			for i := 0; i < n; i++ {
				out += "%" + upperHex(buf[i])
			}
		}
	}
	return out
}

func utf8EncodeRune(buf []byte, r rune) int {
	return copy(buf, []byte(string(r)))
}

func upperHex(b byte) string {
	hex := "0123456789ABCDEF"
	return string([]byte{hex[b>>4], hex[b&0xF]})
}

func (rb *RequestBuilder) Send() *types.Result[HttpResponse, errors.WeilError] {
	args := OutcallRequest{
		Url:         rb.url,
		Method:      string(rb.method),
		Headers:     rb.headers,
		Body:        rb.body,
		QueryParams: rb.queryParams,
	}
	result := types.NewOkResult[OutcallRequest, errors.WeilError](&args)
	buf := internal.LengthPrefixedBytesFromResult(result)
	ptr := internal.GetWasmPtr(&buf[0])
	resultPtr := make_http_outcall(ptr)
	// Use runtime.read to get the response
	respResult := internal.Read(uintptr(resultPtr))
	if respResult.IsErrResult() {
		errMsg := (*respResult.TryErrResult()).Error()
		var err errors.WeilError = errors.NewOutcallError(errMsg)

		return types.NewErrResult[HttpResponse, errors.WeilError](&err)
	}
	okResult := respResult.TryOkResult()
	var httpResponse HttpResponse

	_ = json.Unmarshal(*okResult, &httpResponse)

	return types.NewOkResult[HttpResponse, errors.WeilError](&httpResponse)
}

type HttpResponse struct {
	status uint16
	Body   string
}

func (r *HttpResponse) Text() string {
	return r.Body
}

func (r *HttpResponse) JSON(v any) error {
	return json.Unmarshal([]byte(r.Body), v)
}

func (r *HttpResponse) Status() uint16 {
	return r.status
}
