<?php

declare(strict_types=1);

namespace SearchKernel;

use InvalidArgumentException;

final class IndexDefinition
{
    /**
     * @param array<string,Field> $fields
     * @param list<RankProfile> $rankProfiles
     */
    public function __construct(
        private readonly array $fields,
        private readonly array $rankProfiles = [],
    ) {
        foreach ($fields as $name => $field) {
            if (!is_string($name) || !$field instanceof Field) {
                throw new InvalidArgumentException('index fields must map string names to Field values');
            }
        }
        foreach ($rankProfiles as $profile) {
            if (!$profile instanceof RankProfile) {
                throw new InvalidArgumentException('rank profiles must contain RankProfile values');
            }
        }
    }

    /** @return array<string,mixed> */
    public function toWire(): array
    {
        $fields = $this->fields;
        ksort($fields, SORT_STRING);
        $definitions = [];
        foreach ($fields as $name => $field) {
            $definitions[] = $field->definition($name);
        }

        $result = ['fields' => $definitions];
        if ($this->rankProfiles !== []) {
            $result['rank_profiles'] = array_map(
                static fn (RankProfile $profile): array => $profile->definition(),
                $this->rankProfiles,
            );
        }
        return $result;
    }
}
