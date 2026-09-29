<?php
declare(strict_types=1);
spl_autoload_register(static function(string $class): void { $p='SearchKernel\\'; if(str_starts_with($class,$p)){ $f=dirname(__DIR__).'/src/'.substr($class,strlen($p)).'.php'; if(is_file($f)) require $f; }});
use SearchKernel\Index; use SearchKernel\Protocol; use SearchKernel\Transport; use SearchKernel\UInt64;
final class NoopTransport implements Transport { public function execute(array $request): array { throw new RuntimeException('not executed'); } }

/** @param array<string,mixed> $value */
function parityValue(array $value): mixed
{
    if (array_key_exists('int64', $value)) {
        if (PHP_INT_SIZE < 8) {
            throw new RuntimeException('int64 parity requires 64-bit PHP');
        }
        $decimal = $value['int64'];
        if (!is_string($decimal) || preg_match('/^-?(0|[1-9][0-9]*)$/', $decimal) !== 1) {
            throw new RuntimeException('invalid int64 parity value');
        }
        $integer = filter_var($decimal, FILTER_VALIDATE_INT);
        if ($integer === false) {
            throw new RuntimeException('int64 parity value is outside PHP int range: ' . $decimal);
        }
        return $integer;
    }
    if (array_key_exists('uint64', $value)) {
        return UInt64::fromDecimal($value['uint64']);
    }
    if (array_key_exists('string', $value)) {
        return $value['string'];
    }
    if (array_key_exists('float64', $value)) {
        return $value['float64'];
    }
    throw new RuntimeException('unsupported parity corpus value');
}
$corpus=getenv('SEARCHKERNEL_PARITY_CORPUS'); $output=getenv('SEARCHKERNEL_PARITY_OUTPUT'); $snapshot=getenv('SEARCHKERNEL_SEMANTIC_SNAPSHOT'); $fixture=getenv('SEARCHKERNEL_PARITY_INDEX');
if(!$corpus||!$output||!$snapshot||!$fixture) throw new RuntimeException('missing parity environment'); $data=json_decode(file_get_contents($corpus),true,512,JSON_THROW_ON_ERROR); $index=new Index(new NoopTransport()); $out=[];
foreach($data['scenarios'] as $scenario){ $i=$scenario['intent']; $b=$i['kind']==='browse'?$index->browse():$index->search($i['query']); if(isset($i['filter'])){$bindings=[]; foreach($i['filter']['bindings'] as $k=>$v){$bindings[$k]=parityValue($v);} $b=$b->filter($i['filter']['source'],$bindings);} if(isset($i['ranking']))$b=$b->ranking($i['ranking']); foreach($i['inputs']??[] as $k=>$v){$b=$b->input($k,parityValue($v));} foreach($i['rank_properties']??[] as $k=>$v)$b=$b->rankProperty($k,$v); if(array_key_exists('limit',$i))$b=$b->limit($i['limit']); if(array_key_exists('offset',$i))$b=$b->offset($i['offset']); $tmp=tempnam(sys_get_temp_dir(),'sk-parity-'); file_put_contents($tmp,Protocol::encodeJson($b->request())); $cmd=escapeshellarg($snapshot).' --index-json '.escapeshellarg($fixture).' --request-json '.escapeshellarg($tmp); $json=shell_exec($cmd); unlink($tmp); if($json===null) throw new RuntimeException('snapshot failed'); $out[$scenario['id']]=json_decode($json,true,512,JSON_THROW_ON_ERROR); }
foreach ($data['facade_rejections'] ?? [] as $rejection) {
    if (!in_array('php', $rejection['clients'], true)) {
        continue;
    }
    foreach ($rejection['intent']['rank_properties'] as $name => $value) {
        try {
            $index->browse()->rankProperty($name, $value);
        } catch (TypeError) {
            continue;
        }
        throw new RuntimeException('rank-property rejection accepted: ' . $rejection['id']);
    }
}
file_put_contents($output,json_encode($out,JSON_THROW_ON_ERROR));
