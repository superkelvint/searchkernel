<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class RankProfile
{
    private ?string $firstPhase = null;
    private ?string $secondPhase = null;
    private ?int $rerankCount = null;

    /** @var list<string> */
    private array $matchFeatures = [];

    /** @var array<string,string> */
    private array $queryInputs = [];

    public function __construct(private readonly string $name)
    {
        self::nonEmpty($name, 'rank profile name');
    }

    public function firstPhase(string $expression): self
    {
        self::nonEmpty($expression, 'first-phase expression');
        $this->firstPhase = $expression;
        return $this;
    }

    public function secondPhase(string $expression): self
    {
        self::nonEmpty($expression, 'second-phase expression');
        $this->secondPhase = $expression;
        return $this;
    }

    public function rerankCount(int $count): self
    {
        if ($count < 0 || $count > 0xFFFF_FFFF) {
            throw new InvalidArgumentException('rerank count must be a uint32');
        }
        $this->rerankCount = $count;
        return $this;
    }

    public function matchFeature(string $expression): self
    {
        self::nonEmpty($expression, 'match feature');
        $this->matchFeatures[] = $expression;
        return $this;
    }

    public function queryInput(string $name, string $typeSpec): self
    {
        self::nonEmpty($name, 'query input name');
        self::nonEmpty($typeSpec, 'query input type');
        $this->queryInputs[$name] = $typeSpec;
        return $this;
    }

    /** @return array<string,mixed> */
    public function definition(): array
    {
        $result = ['name' => $this->name];

        if ($this->queryInputs !== []) {
            ksort($this->queryInputs, SORT_STRING);
            $result['inputs'] = [];
            foreach ($this->queryInputs as $name => $typeSpec) {
                $result['inputs'][] = [
                    'name' => $name,
                    'data_type' => [
                        'kind_type' => 'TensorDataType',
                        'kind' => ['type_spec' => $typeSpec],
                    ],
                    'default_value' => Protocol::value(Tensor::dense($typeSpec, [])),
                ];
            }
        }

        if ($this->firstPhase !== null) {
            $result['first_phase'] = ['expression' => $this->firstPhase];
        }
        if ($this->secondPhase !== null || $this->rerankCount !== null) {
            $result['second_phase'] = [];
            if ($this->secondPhase !== null) {
                $result['second_phase']['expression'] = $this->secondPhase;
            }
            if ($this->rerankCount !== null) {
                $result['second_phase']['rerank_count'] = ['value' => $this->rerankCount];
            }
        }
        if ($this->matchFeatures !== []) {
            $result['match_features'] = $this->matchFeatures;
        }

        return $result;
    }

    private static function nonEmpty(string $value, string $label): void
    {
        if (trim($value) === '') {
            throw new InvalidArgumentException($label . ' must be non-empty');
        }
    }
}
