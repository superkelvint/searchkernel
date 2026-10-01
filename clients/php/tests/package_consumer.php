<?php

declare(strict_types=1);

if ($argc !== 2) {
    throw new RuntimeException('usage: package_consumer.php <consumer-dir>');
}

require $argv[1] . '/vendor/autoload.php';

use SearchKernel\Agg;
use SearchKernel\AnalyzeBuilder;
use SearchKernel\Client;
use SearchKernel\CoverageDegraded;
use SearchKernel\Group;
use SearchKernel\MatchedUpdate;
use SearchKernel\Index;
use SearchKernel\Protocol;
use SearchKernel\Transport;
use SearchKernel\Query;
use SearchKernel\UInt64;
use SearchKernel\ValueObject;

foreach ([
    AnalyzeBuilder::class,
    Client::class,
    CoverageDegraded::class,
    Index::class,
    MatchedUpdate::class,
    Protocol::class,
    Query::class,
    Group::class,
    Agg::class,
    UInt64::class,
    ValueObject::class,
] as $class) {
    if (!class_exists($class)) {
        throw new RuntimeException("packaged public class is not autoloadable: {$class}");
    }
}

$value = Protocol::value(new UInt64('18446744073709551615'));
if (($value['kind_type'] ?? null) !== 'UInt64Value') {
    throw new RuntimeException('packaged UInt64 helper did not reach Protocol');
}

$query = Query::contains('title', 'fox');
if (($query['source_type'] ?? null) !== 'TypedQuery') {
    throw new RuntimeException('packaged Query facade is not usable');
}

$grouping = Group::by('brand')->limit(10)->output(Agg::count()->as('products'))->toRequest();
if (($grouping['source_type'] ?? null) !== 'StructuredGroupingSource') {
    throw new RuntimeException('packaged grouping facade is not usable');
}


$transport = new class implements Transport {
    public function execute(array $request): array
    {
        if (($request['payload_type'] ?? null) !== 'SearchKernel_Wire_DescribeRequest') {
            throw new RuntimeException('unexpected packaged low-level request');
        }
        return [
            'protocol_version' => 1,
            'payload_type' => 'SearchKernel_Wire_DescribeResult',
            'payload' => ['index' => ['name' => 'packaged']],
        ];
    }
};
$client = new Client($transport);
if (($client->describe()['index']['name'] ?? null) !== 'packaged') {
    throw new RuntimeException('packaged low-level client is not usable');
}

echo "PHP external Composer consumer PASS\n";
