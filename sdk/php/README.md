# agentspan/sdk (PHP)

Official PHP SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```bash
composer require agentspan/sdk
```

```php
use AgentSpan\Client;

$client = new Client('http://localhost:8080', 'as_...');
$content = $client->read('https://example.com');
echo $content['body'];

$results = $client->search('hackernews', 'rust', 10);
$batch = $client->batchRead(['https://a', 'https://b']);
```

PSR-18/Guzzle based. See [docs/api-reference.md](../../docs/api-reference.md).
Tests: `composer install && composer test` (PHPUnit + Guzzle MockHandler).
