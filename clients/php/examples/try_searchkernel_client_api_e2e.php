<?php

declare(strict_types=1);

spl_autoload_register(static function (string $class): void {
    $prefix = 'SearchKernel\\';
    if (!str_starts_with($class, $prefix)) {
        return;
    }
    $path = dirname(__DIR__) . '/src/' . substr($class, strlen($prefix)) . '.php';
    if (is_file($path)) {
        require $path;
    }
});

use SearchKernel\Index;

function heading(string $title): void
{
    echo "\n", str_repeat('=', 80), "\n", $title, "\n", str_repeat('=', 80), "\n";
}

function check(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$url = getenv('SEARCHKERNEL_HTTP_URL') ?: 'http://127.0.0.1:50052';
$index = Index::connect($url);
$id = 'php-client-e2e-1';

heading('1. UPSERT DOCUMENT');
$written = $index->upsert([
    '_id' => $id,
    'title' => 'The quick brown fox from PHP',
]);
check($written === $id, 'upsert must return the document id');
echo "upsert: {$written}\n";

heading('2. GET BY ID');
$document = $index->get($id);
check($document !== null, 'document must exist');
check($document['_id'] === $id, 'document id must round trip');
check($document['title'] === 'The quick brown fox from PHP', 'stored title must round trip');
print_r($document);

heading('3. FULL-TEXT SEARCH');
$results = $index->search('brown fox')->limit(10)->execute();
$ids = array_map(static fn ($hit) => $hit->id, $results->hits);
check(in_array($id, $ids, true), 'search must return the PHP document');
echo 'total_hits = ', (string) $results->totalHits, ' relation = ', $results->totalHitsRelation, "\n";
foreach ($results->hits as $hit) {
    echo '  id=', $hit->id, ' fields=', json_encode($hit->fields, JSON_UNESCAPED_UNICODE), "\n";
}

heading('4. DELETE');
check($index->delete($id), 'delete must report true for the existing document');
check($index->get($id) === null, 'deleted document must be absent');

echo "\nPHP HTTP/FlatBuffers-JSON E2E PASS\n";
