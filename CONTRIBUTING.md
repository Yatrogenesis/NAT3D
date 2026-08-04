# Contributing to NAT3D

Thank you for your interest in NAT3D.

## What requires signing, and what does not

| Activity | Signature required? |
|---|---|
| Reporting a bug or opening an issue | **No** |
| Downloading, building, or using the binary | **No** |
| Requesting a feature / discussing | **No** |
| **Submitting code via a Pull Request** | **Yes — CLA** |

Using and compiling NAT3D is free under **AGPL-3.0-or-later** (see `LICENSE-AGPL`).
Nothing to sign for that. A signature is required **only** when you contribute
*code* through a Pull Request.

## Why a CLA

NAT3D is dual-licensed (AGPL **or** commercial — see `LICENSE-COMMERCIAL`). When
you submit code and it is merged, you remain the author of your part, but the
project needs clear permission to include that code in **future commercial or
proprietary releases**. The [Contributor License Agreement](CLA.md) grants that
permission once, up front — so the project can be licensed or sold later without
having to re-contact every past contributor. You keep authorship of your work.

## How to sign

1. Open your Pull Request as usual.
2. A bot (CLA Assistant) will comment automatically if a signature is needed.
3. Reply on the PR with the **exact** phrase:

   ```
   I have read the CLA Document and I hereby sign the CLA
   ```

4. The bot records your signature in the `cla-signatures` branch and unblocks
   the merge. You only sign once; it covers all your future contributions.

## Development

```bash
cargo build --release -p nat3d-app
cargo test --workspace
```

See `README.md` for the crate layout and requirements.
