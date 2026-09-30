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

$url = $argv[1] ?? null;
$target = $argv[2] ?? 'php-http-smoke';
if (!is_string($url) || $url === '') {
    fwrite(STDERR, "usage: create_index_http_smoke.php URL [TARGET]\n");
    exit(2);
}

Index::create(
    $url,
    $target,
    searchkernelPhpClientApiE2eDefinition('tensor<float>(x[8])'),
);

echo "PHP HttpTransport create-index PASS\n";
