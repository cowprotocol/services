# Proposed public container CI interface

This accompanies the draft services workflow refactor. `cowprotocol/ci` and its
`v1` release are assumed dependencies; the caller cannot run until they exist.
Before merging, publish/review the shared implementation and replace all `@v1`
references with its full commit SHA, consistent with this repository's Renovate
configuration. No GitHub repository is created by this example.

```text
cowprotocol/ci/
├── .github/
│   └── workflows/
│       ├── build-image.yml       # Native AMD64 + ARM64 builds, then publish
│       └── publish-image.yml     # Merge digest artifacts and apply image tags
├── actions/
│   └── build-image/
│       └── action.yml            # Package one platform and upload its digest
└── README.md
```

## Services flow

```text
services build (native AMD64 + native ARM64)
  Cargo + existing Rust cache in services
  -> shared build-image action packages the matching binaries
  -> services-digest-amd64 / services-digest-arm64 artifacts
publish (shared publish-image.yml)
  -> validate AMD64 + ARM64 descriptors and publish services tags
migrations (shared build-image.yml)
  -> build Dockerfile.deploy-ci target=migrations on both native runners
  -> publish services-migration tags
  -> main-only autodeploy in services
```

Rust toolchain setup, full Git history, restored mtimes, feature selection,
feature-sensitive cache behavior and the package list stay in services. The
shared action consumes the existing `binaries=target/release` local build
context. It must not replace that context with a pristine Git build context.

The migration Dockerfile target only copies SQL onto Flyway. Using the common
native matrix adds two jobs compared with today's single multi-platform build,
but keeps one shared Dockerfile interface for other repositories to adopt.

## Contract

| Entry point | Inputs used by services | Responsibilities |
| --- | --- | --- |
| `actions/build-image` | `image`, `platform`, `context`, `file`, `build-contexts`, `build-args`, `labels`, `digest-artifact-name`, `registry-token` | No checkout; use caller's working directory, confirm native platform, log into GHCR, set up Buildx, generate labels, push by digest without tags, upload digest artifact; return `digest`. |
| `publish-image.yml` | `image`, `digest-artifact-pattern`, `tags`, `labels` | Download only the current run's matching artifacts, require one AMD64 and one ARM64 image, preserve attestation descriptors/index annotations, publish the combined index and return its digest. |
| `build-image.yml` | `image`, `context`, `file`, `target`, `artifact-prefix`, `tags`, `labels` | Checkout caller source on both native runners, call the single-platform action and publish only after both builds succeed; return the index digest. |

The reusable workflows use the caller's `GITHUB_TOKEN`, with `contents: read`
and `packages: write`. They do not receive the deployment secrets. The composite
action takes the registry token explicitly because actions cannot use the
`secrets` context directly.

Artifact prefixes distinguish images inside a single run. Runtime platforms
must be checked explicitly; attestation entries with `unknown/unknown` do not
count. Keep existing SHA/branch/tag metadata rules and GPL labels. Neither a
plain single-platform image nor an incomplete matrix may advance the services
tag. `autodeploy` waits for the migration workflow to finish, so both images
must be published before a deployment starts.

This example preserves the existing services-then-migrations publication order;
it does not make publication across two image names transactional. It also does
not introduce application runtime tests, registry cache policy, PR publishing
or a general release framework. Those are follow-up capabilities of the shared
CI repository. The existing services Cargo cache is preserved.

## Validation before merging

1. Publish the shared files and pin their actual commit SHA in this caller.
2. Run the caller from a disposable branch or test repository with separate
   GHCR image names, so validation does not move production image tags.
3. Confirm both runtime platforms in services and migration image indexes,
   matching image labels and index annotations, preserved provenance, and that
   feature builds contain the intended binaries.
4. Exercise an architecture build failure: publication must remain blocked.
5. Exercise a migration failure: autodeploy must remain blocked.
6. Confirm the native AMD64 and ARM64 images execute successfully before enabling
   the production caller.
