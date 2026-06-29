<?php

declare(strict_types=1);

namespace AgentSpan;

/** Base class for all AgentSpan SDK errors. */
class AgentSpanException extends \Exception {}

/** Raised on HTTP 401. */
class AuthenticationException extends AgentSpanException {}

/** Raised on HTTP 429. */
class RateLimitException extends AgentSpanException
{
    public ?int $retryAfter;

    public function __construct(string $message, ?int $retryAfter = null)
    {
        parent::__construct($message);
        $this->retryAfter = $retryAfter;
    }
}

/** Raised for other non-2xx responses. */
class ApiException extends AgentSpanException
{
    public int $status;

    public function __construct(string $message, int $status)
    {
        parent::__construct($message);
        $this->status = $status;
    }
}

/** Raised when the server embeds {"error":...} in a 200 body. */
class ChannelException extends AgentSpanException {}
