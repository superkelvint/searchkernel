<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class Field
{
    /**
     * @param array<string,mixed> $dataType
     * @param array<string,?bool> $capabilities
     */
    private function __construct(
        private readonly array $dataType,
        private readonly ?string $matching,
        private readonly array $capabilities,
    ) {}

    public static function text(
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return self::primitive('String', 'Text', $stored, $searchable, $filterable, $sortable, $facetable);
    }

    public static function keyword(
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return self::primitive('String', 'Exact', $stored, $searchable, $filterable, $sortable, $facetable);
    }

    public static function int64(
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return self::primitive('Int64', null, $stored, $searchable, $filterable, $sortable, $facetable);
    }

    public static function float64(
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return self::primitive('Float64', null, $stored, $searchable, $filterable, $sortable, $facetable);
    }

    public static function boolean(
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return self::primitive('Bool', null, $stored, $searchable, $filterable, $sortable, $facetable);
    }

    public static function tensor(
        string $typeSpec,
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        self::nonEmpty($typeSpec, 'tensor type spec');
        return new self(
            [
                'kind_type' => 'TensorDataType',
                'kind' => ['type_spec' => $typeSpec],
            ],
            null,
            self::capabilities($stored, $searchable, $filterable, $sortable, $facetable),
        );
    }

    public static function position(
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return new self(
            ['kind_type' => 'PositionDataType', 'kind' => JsonObject::empty()],
            null,
            self::capabilities($stored, $searchable, $filterable, $sortable, $facetable),
        );
    }

    /** @param array<string,Field> $fields */
    public static function structOf(
        array $fields,
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        if ($fields === []) {
            throw new InvalidArgumentException('struct fields must be non-empty');
        }
        ksort($fields, SORT_STRING);
        $members = [];
        foreach ($fields as $name => $field) {
            if (!is_string($name) || !$field instanceof self) {
                throw new InvalidArgumentException('struct fields must map string names to Field values');
            }
            self::nonEmpty($name, 'struct field name');
            $members[] = ['name' => $name, 'data_type' => $field->dataType()];
        }
        return new self(
            [
                'kind_type' => 'StructDataType',
                'kind' => ['fields' => $members],
            ],
            null,
            self::capabilities($stored, $searchable, $filterable, $sortable, $facetable),
        );
    }

    public static function arrayOf(
        self $element,
        ?bool $stored = null,
        ?bool $searchable = null,
        ?bool $filterable = null,
        ?bool $sortable = null,
        ?bool $facetable = null,
    ): self {
        return new self(
            [
                'kind_type' => 'ArrayDataType',
                'kind' => ['element_type' => $element->dataType()],
            ],
            null,
            self::capabilities($stored, $searchable, $filterable, $sortable, $facetable),
        );
    }

    /** @return array<string,mixed> */
    public function definition(string $name): array
    {
        self::nonEmpty($name, 'field name');
        $result = ['name' => $name, 'data_type' => $this->dataType];
        foreach ($this->capabilities as $key => $value) {
            if ($value !== null) {
                $result[$key] = ['value' => $value];
            }
        }
        if ($this->matching !== null) {
            $result['matching'] = ['mode' => $this->matching];
        }
        return $result;
    }

    /** @return array<string,mixed> */
    public function dataType(): array
    {
        return $this->dataType;
    }

    private static function primitive(
        string $kind,
        ?string $matching,
        ?bool $stored,
        ?bool $searchable,
        ?bool $filterable,
        ?bool $sortable,
        ?bool $facetable,
    ): self {
        return new self(
            [
                'kind_type' => 'PrimitiveDataType',
                'kind' => ['kind' => $kind],
            ],
            $matching,
            self::capabilities($stored, $searchable, $filterable, $sortable, $facetable),
        );
    }

    /** @return array<string,?bool> */
    private static function capabilities(
        ?bool $stored,
        ?bool $searchable,
        ?bool $filterable,
        ?bool $sortable,
        ?bool $facetable,
    ): array {
        return [
            'stored' => $stored,
            'searchable' => $searchable,
            'filterable' => $filterable,
            'sortable' => $sortable,
            'facetable' => $facetable,
        ];
    }

    private static function nonEmpty(string $value, string $label): void
    {
        if (trim($value) === '') {
            throw new InvalidArgumentException($label . ' must be non-empty');
        }
    }
}
