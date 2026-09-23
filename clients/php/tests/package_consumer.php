<?php

declare(strict_types=1);

if ($argc !== 2) {
    throw new RuntimeException('usage: package_consumer.php <consumer-dir>');
}

require $argv[1] . '/vendor/autoload.php';

use SearchKernel\Agg;
use SearchKernel\AnalyzeBuilder;
use SearchKernel\CoverageDegraded;
use SearchKernel\Group;
use SearchKernel\MatchedUpdate;
use SearchKernel\Index;
use SearchKernel\Protocol;
use SearchKernel\Query;
use SearchKernel\UInt64;
use SearchKernel\ValueObject;

foreach ([
    AnalyzeBuilder::class,
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

echo "PHP external Composer consumer PASS\n";
