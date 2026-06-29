// Package agentspan is the official Go SDK for the AgentSpan gateway.
//
// It is a thin client over the AgentSpan REST API (see docs/api-reference.md).
package agentspan

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
)

// Content is returned by read operations.
type Content struct {
	URL      string          `json:"url"`
	Title    *string         `json:"title"`
	Body     string          `json:"body"`
	Metadata json.RawMessage `json:"metadata"`
	Cached   bool            `json:"cached"`
}

// SearchResult is a single search hit.
type SearchResult struct {
	Title     string  `json:"title"`
	URL       string  `json:"url"`
	Snippet   string  `json:"snippet"`
	Author    *string `json:"author"`
	Timestamp *string `json:"timestamp"`
}

// ChannelInfo is channel metadata.
type ChannelInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Tier        string `json:"tier"`
}

// Error categories.

// AuthError is returned on HTTP 401.
type AuthError struct{ Message string }

func (e *AuthError) Error() string { return "authentication failed: " + e.Message }

// RateLimitError is returned on HTTP 429.
type RateLimitError struct{ RetryAfter int }

func (e *RateLimitError) Error() string {
	return fmt.Sprintf("rate limited (retry after %ds)", e.RetryAfter)
}

// APIError is returned for other non-2xx responses.
type APIError struct {
	Status  int
	Message string
}

func (e *APIError) Error() string { return fmt.Sprintf("API error %d: %s", e.Status, e.Message) }

// ChannelError is returned when the server embeds {"error":...} in a 200 body.
type ChannelError struct{ Message string }

func (e *ChannelError) Error() string { return "channel error: " + e.Message }

// Client is an AgentSpan API client.
type Client struct {
	BaseURL string
	APIKey  string
	HTTP    *http.Client
}

// New creates a client pointed at baseURL (e.g. http://localhost:8080).
func New(baseURL string) *Client {
	return &Client{
		BaseURL: strings.TrimRight(baseURL, "/"),
		HTTP:    http.DefaultClient,
	}
}

// WithAPIKey sets the X-API-Key header value.
func (c *Client) WithAPIKey(key string) *Client {
	c.APIKey = key
	return c
}

func (c *Client) do(ctx context.Context, method, path string, query url.Values, body any) (json.RawMessage, error) {
	u := c.BaseURL + path
	if len(query) > 0 {
		u += "?" + query.Encode()
	}
	var reader io.Reader
	if body != nil {
		b, err := json.Marshal(body)
		if err != nil {
			return nil, err
		}
		reader = bytes.NewReader(b)
	}
	req, err := http.NewRequestWithContext(ctx, method, u, reader)
	if err != nil {
		return nil, err
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	if c.APIKey != "" {
		req.Header.Set("X-API-Key", c.APIKey)
	}
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	data, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		return nil, classify(resp, data)
	}
	return data, nil
}

func classify(resp *http.Response, data []byte) error {
	msg := errorMessage(data, resp.Status)
	switch resp.StatusCode {
	case 401:
		return &AuthError{Message: msg}
	case 429:
		ra, _ := strconv.Atoi(resp.Header.Get("Retry-After"))
		return &RateLimitError{RetryAfter: ra}
	default:
		return &APIError{Status: resp.StatusCode, Message: msg}
	}
}

func errorMessage(data []byte, fallback string) string {
	var env struct {
		Error string `json:"error"`
	}
	if json.Unmarshal(data, &env) == nil && env.Error != "" {
		return env.Error
	}
	return fallback
}

// Read reads a URL via the best matching channel.
func (c *Client) Read(ctx context.Context, target string, forceRefresh bool) (*Content, error) {
	q := url.Values{"url": {target}, "force_refresh": {strconv.FormatBool(forceRefresh)}}
	data, err := c.do(ctx, http.MethodGet, "/api/v1/read", q, nil)
	if err != nil {
		return nil, err
	}
	var env struct {
		Error   string  `json:"error"`
		Content Content `json:"content"`
	}
	if err := json.Unmarshal(data, &env); err != nil {
		return nil, err
	}
	if env.Error != "" {
		return nil, &ChannelError{Message: env.Error}
	}
	return &env.Content, nil
}

// Search runs a query against a named channel.
func (c *Client) Search(ctx context.Context, channel, query string, limit int) ([]SearchResult, error) {
	q := url.Values{"q": {query}, "limit": {strconv.Itoa(limit)}}
	data, err := c.do(ctx, http.MethodGet, "/api/v1/channels/"+channel+"/search", q, nil)
	if err != nil {
		return nil, err
	}
	var env struct {
		Error   string         `json:"error"`
		Results []SearchResult `json:"results"`
	}
	if err := json.Unmarshal(data, &env); err != nil {
		return nil, err
	}
	if env.Error != "" {
		return nil, &ChannelError{Message: env.Error}
	}
	return env.Results, nil
}

// ListChannels returns the available channels.
func (c *Client) ListChannels(ctx context.Context) ([]ChannelInfo, error) {
	data, err := c.do(ctx, http.MethodGet, "/api/v1/channels", nil, nil)
	if err != nil {
		return nil, err
	}
	var env struct {
		Channels []ChannelInfo `json:"channels"`
	}
	err = json.Unmarshal(data, &env)
	return env.Channels, err
}

// Doctor runs health diagnostics across all channels.
func (c *Client) Doctor(ctx context.Context) (map[string]any, error) {
	data, err := c.do(ctx, http.MethodGet, "/api/v1/doctor", nil, nil)
	if err != nil {
		return nil, err
	}
	var out map[string]any
	err = json.Unmarshal(data, &out)
	return out, err
}

// GetConfig returns the non-secret configuration view.
func (c *Client) GetConfig(ctx context.Context) (map[string]any, error) {
	data, err := c.do(ctx, http.MethodGet, "/api/v1/config", nil, nil)
	if err != nil {
		return nil, err
	}
	var out map[string]any
	err = json.Unmarshal(data, &out)
	return out, err
}

// BatchRead reads many URLs in parallel (server-side batch).
func (c *Client) BatchRead(ctx context.Context, urls []string, forceRefresh bool) ([]json.RawMessage, error) {
	body := map[string]any{"urls": urls, "force_refresh": forceRefresh}
	data, err := c.do(ctx, http.MethodPost, "/api/v1/batch/read", nil, body)
	if err != nil {
		return nil, err
	}
	var env struct {
		Results []json.RawMessage `json:"results"`
	}
	err = json.Unmarshal(data, &env)
	return env.Results, err
}

// BatchSearch runs many queries against one channel in parallel.
func (c *Client) BatchSearch(ctx context.Context, channel string, queries []string, limit int) ([]json.RawMessage, error) {
	body := map[string]any{"channel": channel, "queries": queries, "limit": limit}
	data, err := c.do(ctx, http.MethodPost, "/api/v1/batch/search", nil, body)
	if err != nil {
		return nil, err
	}
	var env struct {
		Results []json.RawMessage `json:"results"`
	}
	err = json.Unmarshal(data, &env)
	return env.Results, err
}

// Health reports whether the server's /health endpoint is OK.
func (c *Client) Health(ctx context.Context) bool {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.BaseURL+"/health", nil)
	if err != nil {
		return false
	}
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	return resp.StatusCode == 200
}
