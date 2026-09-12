# Deferred from v0.4

The v0.4 corpus had become a product-roadmap corpus rather than an API→IR corpus. v0.5 deliberately removes the following from active acceptance:

- first-class bulk execution semantics;
- browse as a separate operation;
- grouping/distinct;
- presets/search settings;
- synonyms and synonym expansion;
- curations;
- lifecycle/open/create/close management;
- analyzer execution/tokenization;
- backend result mapping while the owned result model is being frozen;
- prefix/typo execution beyond the current fixed `Disabled` IR policies;
- old required/optional document-field semantics and patch operations not in the current portable contract.

No case was silently forgotten: every v0.4 case is classified in `migration/v0.4-classification.json`.
