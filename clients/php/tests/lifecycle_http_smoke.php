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

require dirname(__DIR__) . '/examples/searchkernel_client_api_e2e_schema.php';

use SearchKernel\Index;
use SearchKernel\SearchKernelException;

$url = $argv[1] ?? null;
if (!is_string($url) || $url === '') {
    fwrite(STDERR, "usage: lifecycle_http_smoke.php URL\n");
    exit(2);
}

$definition = searchkernelPhpClientApiE2eDefinition(searchkernelPhpClientApiE2eCorpus());

$expectPortable = static function (
    callable $operation,
    string $code,
    string $detail,
    ?string $path,
    string $label,
): void {
    try {
        $operation();
    } catch (SearchKernelException $error) {
        $actual = $error->details[0] ?? [];
        if ($error->errorCode !== $code
            || ($actual['code'] ?? null) !== $detail
            || ($actual['path'] ?? null) !== $path) {
            throw new RuntimeException($label . ' returned non-canonical lifecycle semantics');
        }
        return;
    }
    throw new RuntimeException($label . ' unexpectedly succeeded');
};

foreach (["", 'Products', '../products', str_repeat('a', 129)] as $invalidTarget) {
    foreach ([
        static fn () => Index::create($url, $invalidTarget, $definition),
        static fn () => Index::open($url, $invalidTarget),
        static fn () => Index::deleteTarget($url, $invalidTarget),
    ] as $operation) {
        $expectPortable(
            $operation,
            'InvalidArgument',
            'invalid_index_target',
            'target.name',
            'invalid target lifecycle request',
        );
    }
}

$boundaryTarget = str_repeat('a', 128);
Index::create($url, $boundaryTarget, $definition);
Index::open($url, $boundaryTarget);
if (!Index::deleteTarget($url, $boundaryTarget)) {
    throw new RuntimeException('128-byte target was not deleted');
}

$target = 'php-lifecycle-smoke';
Index::create($url, $target, $definition);
$expectPortable(
    static fn () => Index::create($url, $target, $definition),
    'AlreadyExists',
    'index_already_exists',
    null,
    'duplicate create',
);
Index::open($url, $target);
if (!Index::deleteTarget($url, $target)) {
    throw new RuntimeException('created target was not deleted');
}
$expectPortable(
    static fn () => Index::open($url, $target),
    'NotFound',
    'index_not_found',
    null,
    'open after delete',
);
$expectPortable(
    static fn () => Index::deleteTarget($url, $target),
    'NotFound',
    'index_not_found',
    null,
    'delete after delete',
);

echo "PHP create/open/delete lifecycle semantics PASS\n";
