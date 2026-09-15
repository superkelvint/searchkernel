# Search Kernel web

The Search Kernel Admin Dashboard is a React, TypeScript, Vite, and Tailwind
application for inspecting and operating local Search Kernel indexes.

Development uses the Vite server with `/api` proxied to `searchkerneld`.
Production builds produce a static `dist/` artifact that can either be served
by `searchkerneld` or deployed independently.

Portable operations use the generated TypeScript Search Kernel client. Process
diagnostics use a separate admin client. The frontend does not reimplement
defaults, validation, filtering, ranking, schema capabilities, or request
normalization.
