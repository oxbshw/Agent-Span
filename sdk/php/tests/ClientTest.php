<?php

declare(strict_types=1);

namespace AgentSpan\Tests;

use AgentSpan\AuthenticationException;
use AgentSpan\ChannelException;
use AgentSpan\Client;
use AgentSpan\RateLimitException;
use GuzzleHttp\Client as Http;
use GuzzleHttp\Handler\MockHandler;
use GuzzleHttp\HandlerStack;
use GuzzleHttp\Psr7\Response;
use PHPUnit\Framework\TestCase;

final class ClientTest extends TestCase
{
    /** @param list<Response> $responses */
    private function client(array $responses): Client
    {
        $stack = HandlerStack::create(new MockHandler($responses));
        return new Client('http://test', 'k', new Http(['handler' => $stack]));
    }

    public function testReadReturnsContent(): void
    {
        $c = $this->client([new Response(200, [], json_encode(['channel' => 'web', 'content' => ['body' => 'hi']]))]);
        $this->assertSame('hi', $c->read('https://x')['body']);
    }

    public function testReadChannelError(): void
    {
        $c = $this->client([new Response(200, [], json_encode(['error' => 'no channel']))]);
        $this->expectException(ChannelException::class);
        $c->read('ftp://x');
    }

    public function testAuthError(): void
    {
        $c = $this->client([new Response(401, [], json_encode(['error' => 'bad']))]);
        $this->expectException(AuthenticationException::class);
        $c->listChannels();
    }

    public function testRateLimitCarriesRetryAfter(): void
    {
        $c = $this->client([new Response(429, ['Retry-After' => '12'], json_encode(['error' => 'slow']))]);
        try {
            $c->read('https://x');
            $this->fail('expected RateLimitException');
        } catch (RateLimitException $e) {
            $this->assertSame(12, $e->retryAfter);
        }
    }

    public function testBatchRead(): void
    {
        $c = $this->client([new Response(200, [], json_encode(['results' => [['ok' => true], ['ok' => false]]]))]);
        $this->assertCount(2, $c->batchRead(['a', 'b']));
    }

    public function testHealth(): void
    {
        $c = $this->client([new Response(200, [], '{}')]);
        $this->assertTrue($c->health());
    }
}
