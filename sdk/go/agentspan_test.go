package agentspan

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
)

func server(handler http.HandlerFunc) (*httptest.Server, *Client) {
	srv := httptest.NewServer(handler)
	return srv, New(srv.URL)
}

func TestRead(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/v1/read" {
			t.Fatalf("unexpected path %s", r.URL.Path)
		}
		w.Write([]byte(`{"channel":"web","content":{"url":"https://x","title":"T","body":"hi","cached":false}}`))
	})
	defer srv.Close()
	content, err := client.Read(context.Background(), "https://x", false)
	if err != nil {
		t.Fatal(err)
	}
	if content.Body != "hi" {
		t.Fatalf("got body %q", content.Body)
	}
}

func TestReadChannelError(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte(`{"error":"no channel"}`))
	})
	defer srv.Close()
	_, err := client.Read(context.Background(), "ftp://x", false)
	if _, ok := err.(*ChannelError); !ok {
		t.Fatalf("expected ChannelError, got %v", err)
	}
}

func TestSearch(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/v1/channels/hackernews/search" {
			t.Fatalf("unexpected path %s", r.URL.Path)
		}
		if r.URL.Query().Get("limit") != "7" {
			t.Fatalf("expected limit=7")
		}
		w.Write([]byte(`{"results":[{"title":"Rust","url":"https://r","snippet":"s"}]}`))
	})
	defer srv.Close()
	results, err := client.Search(context.Background(), "hackernews", "rust", 7)
	if err != nil {
		t.Fatal(err)
	}
	if results[0].Title != "Rust" {
		t.Fatalf("got %q", results[0].Title)
	}
}

func TestAuthError(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(401)
		w.Write([]byte(`{"error":"bad key"}`))
	})
	defer srv.Close()
	_, err := client.ListChannels(context.Background())
	if _, ok := err.(*AuthError); !ok {
		t.Fatalf("expected AuthError, got %v", err)
	}
}

func TestRateLimit(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Retry-After", "12")
		w.WriteHeader(429)
		w.Write([]byte(`{"error":"slow"}`))
	})
	defer srv.Close()
	_, err := client.Read(context.Background(), "https://x", false)
	rl, ok := err.(*RateLimitError)
	if !ok || rl.RetryAfter != 12 {
		t.Fatalf("expected RateLimitError(12), got %v", err)
	}
}

func TestBatchRead(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Fatalf("expected POST")
		}
		w.Write([]byte(`{"count":2,"results":[{"url":"a","ok":true},{"url":"b","ok":false}]}`))
	})
	defer srv.Close()
	results, err := client.BatchRead(context.Background(), []string{"a", "b"}, false)
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 2 {
		t.Fatalf("expected 2 results, got %d", len(results))
	}
}

func TestHealth(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
	})
	defer srv.Close()
	if !client.Health(context.Background()) {
		t.Fatal("expected healthy")
	}
}

func TestSendsAPIKey(t *testing.T) {
	srv, client := server(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("X-API-Key") != "k" {
			t.Fatalf("missing api key header")
		}
		w.Write([]byte(`{"channels":[]}`))
	})
	defer srv.Close()
	client.WithAPIKey("k")
	if _, err := client.ListChannels(context.Background()); err != nil {
		t.Fatal(err)
	}
}
