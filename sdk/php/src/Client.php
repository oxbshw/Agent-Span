<?php

declare(strict_types=1);

namespace AgentSpan;

use GuzzleHttp\Client as Http;
use GuzzleHttp\ClientInterface;
use Psr\Http\Message\ResponseInterface;

/**
 * Official PHP client for the AgentSpan gateway.
 *
 * Thin wrapper over the AgentSpan REST API (see docs/api-reference.md).
 */
final class Client
{
    private string $baseUrl;
    private ?string $apiKey;
    private ClientInterface $http;

    public function __construct(
        string $baseUrl = 'http://localhost:8080',
        ?string $apiKey = null,
        ?ClientInterface $http = null
    ) {
        $this->baseUrl = rtrim($baseUrl, '/');
        $this->apiKey = $apiKey;
        $this->http = $http ?? new Http();
    }

    /** @return array<string,mixed> The content object. */
    public function read(string $url, bool $forceRefresh = false): array
    {
        $data = $this->request('GET', '/api/v1/read', [
            'query' => ['url' => $url, 'force_refresh' => $forceRefresh ? 'true' : 'false'],
        ]);
        if (isset($data['error'])) {
            throw new ChannelException($data['error']);
        }
        return $data['content'];
    }

    /** @return list<array<string,mixed>> */
    public function search(string $channel, string $query, int $limit = 10): array
    {
        $data = $this->request('GET', "/api/v1/channels/$channel/search", [
            'query' => ['q' => $query, 'limit' => $limit],
        ]);
        if (isset($data['error'])) {
            throw new ChannelException($data['error']);
        }
        return $data['results'] ?? [];
    }

    /** @return list<array<string,mixed>> */
    public function listChannels(): array
    {
        return $this->request('GET', '/api/v1/channels')['channels'] ?? [];
    }

    /** @return array<string,mixed> */
    public function doctor(): array
    {
        return $this->request('GET', '/api/v1/doctor');
    }

    /** @return array<string,mixed> */
    public function getConfig(): array
    {
        return $this->request('GET', '/api/v1/config');
    }

    /** @return list<array<string,mixed>> */
    public function batchRead(array $urls, bool $forceRefresh = false): array
    {
        return $this->request('POST', '/api/v1/batch/read', [
            'json' => ['urls' => $urls, 'force_refresh' => $forceRefresh],
        ])['results'] ?? [];
    }

    /** @return list<array<string,mixed>> */
    public function batchSearch(string $channel, array $queries, int $limit = 10): array
    {
        return $this->request('POST', '/api/v1/batch/search', [
            'json' => ['channel' => $channel, 'queries' => $queries, 'limit' => $limit],
        ])['results'] ?? [];
    }

    public function health(): bool
    {
        try {
            $res = $this->http->request('GET', $this->baseUrl . '/health', [
                'headers' => $this->headers(),
                'http_errors' => false,
            ]);
            return $res->getStatusCode() === 200;
        } catch (\Throwable) {
            return false;
        }
    }

    /** @return array<string,mixed> */
    public function createKey(string $name, array $scopes = ['read'], string $tenantId = 'default'): array
    {
        return $this->request('POST', '/api/v1/auth/keys', [
            'json' => ['name' => $name, 'scopes' => $scopes, 'tenant_id' => $tenantId],
        ]);
    }

    public function revokeKey(string $id): void
    {
        $this->request('DELETE', "/api/v1/auth/keys/$id");
    }

    /** @return array<string,string> */
    private function headers(): array
    {
        $h = ['Accept' => 'application/json'];
        if ($this->apiKey !== null) {
            $h['X-API-Key'] = $this->apiKey;
        }
        return $h;
    }

    /**
     * @param array<string,mixed> $opts
     * @return array<string,mixed>
     */
    private function request(string $method, string $path, array $opts = []): array
    {
        $opts['headers'] = array_merge($this->headers(), $opts['headers'] ?? []);
        $opts['http_errors'] = false;
        $res = $this->http->request($method, $this->baseUrl . $path, $opts);
        $status = $res->getStatusCode();
        $body = (string) $res->getBody();
        $data = $body === '' ? [] : (json_decode($body, true) ?? []);
        if ($status >= 400) {
            $this->throwError($status, $data, $res);
        }
        return $data;
    }

    /** @param array<string,mixed> $data */
    private function throwError(int $status, array $data, ResponseInterface $res): never
    {
        $message = $data['error'] ?? "HTTP $status";
        if ($status === 401) {
            throw new AuthenticationException($message);
        }
        if ($status === 429) {
            $retry = $res->getHeaderLine('Retry-After');
            throw new RateLimitException($message, $retry === '' ? null : (int) $retry);
        }
        throw new ApiException($message, $status);
    }
}
