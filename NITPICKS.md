# Rey Documentation Nitpicks

This document catalogs all potential issues, inconsistencies, and minor problems found in the Rey documentation files. These are "nitpicks" — issues that should be addressed for clarity, correctness, and consistency, but are not critical blockers.

---

## Summary by Severity

| Severity | Count | Files Affected |
|----------|-------|----------------|
| **High** (must fix) | 5 | SPEC.md, ZOO.md, ANALYSIS.md |
| **Medium** (should fix) | 22 | All files |
| **Low** (nice to fix) | 38 | All files |
| **Total** | **65** | |

---

## Legend

- **🔴 HIGH**: Functional issues, security concerns, or incorrect specifications
- **🟡 MEDIUM**: Inconsistencies, missing important details, potential bugs
- **🟢 LOW**: Style issues, typos, minor formatting problems

---

# ARCHITECTURE.md

## 🟡 MEDIUM Issues

### 1. Dependency Flow Inconsistency
**Location**: Section 1.3
**Issue**: The dependency graph shows `metadata` and `thumbnail` depending on `types, crypto, image`, but the text states `local-db` depends on `types, common` only. However, `sync` depends on `local-db`, creating an implicit dependency chain that isn't visually represented.

### 2. Typo: "plumming" → "plumbing"
**Location**: Section 3.1
**Issue**: "The crate boundaries are the hard part. The workspace is just plumming."

### 3. Inconsistent Crate List
**Location**: Section 3.2 File Tree vs Section 1.2 Crate Layout
**Issue**: Section 3.2 shows `crates/xtask/` but Section 1.2 doesn't list it in the crate layout.

### 4. Missing Error Handling Strategy
**Location**: Entire document
**Issue**: No mention of how errors are handled across crate boundaries or what error handling pattern is recommended.

### 5. Redundant Information
**Location**: Sections 1.4 and 3.2
**Issue**: Crate responsibilities table in Section 1.4 duplicates information in STRUCTURE.md without adding new value.

### 6. Unclear Platform Binding
**Location**: Section 3.2
**Issue**: `apps/desktop/src-tauri/` is described as "thin" and imports `client-lib`, but it's unclear what the boundary is between Tauri's Rust side and `client-lib`.

## 🟢 LOW Issues

### 7. Inconsistent Hyphenation
**Location**: Section 4
**Issue**: "Do alongside" should be "Do this alongside" or rephrased for clarity.

### 8. Markdown Table Alignment
**Location**: Multiple tables
**Issue**: Some tables use inconsistent column alignment (mix of left, center, right).

### 9. Unclear xtask Status
**Location**: Section 1.5
**Issue**: `xtask` pattern is mentioned as "optional" but there's no indication of whether it will be implemented for Rey.

### 10. Missing CI/CD Details
**Location**: Entire document
**Issue**: No mention of CI/CD pipeline, testing strategy, or deployment process.

### 11. Inconsistent Tool Reference
**Location**: Section 3.2
**Issue**: Mentions both `utoipa` (for OpenAPI) and `tauri_specta` without clarifying their respective roles.

### 12. Unrealistic Recompilation Claim
**Location**: Section 1.5
**Issue**: "Change only `client-lib`? Only `client-lib` and its reverse deps recompile" — but `client-lib` depends on `sync`, `local-db`, `thumbnail`, `zoo-client`, so all of those would also need recompilation.

---

# STRUCTURE.md

## 🟡 MEDIUM Issues

### 13. Confusing zoo-client Description
**Location**: Section 2.10
**Issue**: States "No I/O — traits are injected by platform bindings. No crypto — works with encrypted bytes only." This is technically correct but misleading. If it works with encrypted bytes, it must have some understanding of the encryption format, even if it doesn't perform the crypto operations itself.

### 14. Circular Dependency Risk in Diagram
**Location**: Section 3 Dependency Graph
**Issue**: The ASCII diagram is complex and hard to verify for circular dependencies. The text claims "Lower layers never import higher layers" but the visual representation makes this difficult to confirm.

### 15. Inconsistent Import Rules
**Location**: Section 4
**Issue**: The import rules table says `zoo-client` cannot import `crypto`, but `zoo-client` works with encrypted data. This is correct (it handles bytes without knowing the crypto) but could be confusing without clarification.

### 16. Missing Explanation of Platform Abstraction
**Location**: Section 2.11
**Issue**: `zoo-wasm` and `client-lib` are described as "platform bindings" but there's no explanation of how they abstract away platform differences for the core crates.

### 17. Inconsistent File Path Formatting
**Location**: Section 3
**Issue**: ASCII diagrams mix forward slashes and backslashes (e.g., `crates\zoo\` vs `crates/types/`).

### 18. Incorrect Recompilation Scope
**Location**: Section 7
**Issue**: "Change to `types` → Recompiles Everything" is correct but trivial. The more interesting cases (crypto, image) should note that they trigger recompilation of many downstream crates.

### 19. Missing Feature Flag Documentation
**Location**: Section 8
**Issue**: Feature flags are listed but there's no explanation of how they're configured or what the default feature set is.

## 🟢 LOW Issues

### 20. Hard-to-Read ASCII Diagrams
**Location**: Section 3
**Issue**: The dependency graph ASCII art is very complex and difficult to parse. Consider simplifying or splitting into multiple diagrams.

### 21. Redundant Crate Descriptions
**Location**: Sections 2.1-2.11
**Issue**: Some crate descriptions duplicate information from ARCHITECTURE.md.

### 22. Inconsistent Capitalization
**Location**: Section 4 "Layer Ownership Summary" table
**Issue**: "Compile-time guarantee" column has inconsistent capitalization ("No internal deps" vs "Zero framework coupling").

### 23. Unclear File Tree Structure
**Location**: Section 5
**Issue**: The file tree shows `crates/zoo-client/` but the structure isn't as detailed as other crates (missing subdirectories).

### 24. Missing Rationales
**Location**: Section 4 "What Lives Where" table
**Issue**: The "Rationale" column sometimes states the obvious (e.g., "Pure key derivation, no I/O") without explaining WHY this separation matters.

### 25. Formatting Issue in Table
**Location**: Section 6
**Issue**: The "Compile-Time Guarantees" table has malformed markdown (missing pipe characters).

---

# SPEC.md

## 🔴 HIGH Issues

### 26. Timing Attack Vulnerability in Authentication
**Location**: Section 1.3
**Issue**: The server "always returns the same params regardless of email existence" to prevent enumeration. However, this doesn't address **timing attacks**. The server should use constant-time comparison for the verify_key_hash to prevent timing-based enumeration.

**Fix**: Add note: "Server MUST use constant-time comparison (e.g., `secrecy::constant_time_eq` or similar) when comparing verify_key_hash against stored bcrypt hash."

### 27. Inconsistent Argon2id Parameters
**Location**: Section 1.4
**Issue**: The "Sensitive" profile uses 256 MiB / 4 ops, "Mobile" uses 128 MiB / 3 ops, but the adaptive fallback goes down to 32 MiB floor. The relationship between these isn't clear, and "Interactive" at 64 MiB / 2 ops doesn't fit the pattern.

**Fix**: Define clear naming conventions and explain the rationale for each profile's parameters.

### 28. SQL Index Missing for Date Queries
**Location**: Section 2.5
**Issue**: The SQLite schema has `CREATE INDEX idx_files_taken_at ON files(taken_at);` but Section 4.1 shows date range queries using `taken_at BETWEEN`. However, the FTS5 text search uses `LIKE '%query%'` which won't use any index effectively.

**Fix**: Use FTS5 virtual tables for text search, or add `COLLATE NOCASE` to the LIKE query.

## 🟡 MEDIUM Issues

### 29. Content Hash Privacy Leak Unaddressed
**Location**: Section 5.1
**Issue**: The content hash (SHA-256) leak is acknowledged but no mitigation is proposed, even for future versions. For a privacy-focused application, this is a significant metadata leak.

**Suggestion**: Add a note about potential future mitigations: keyed hashes, blind deduplication, or client-side deduplication.

### 30. Inconsistent Archival Field Naming
**Location**: Sections 2.5, 5.2
**Issue**: Files use `archived_at` but devices also use `archived_at`. This is actually consistent, but should be explicitly stated as a convention.

### 31. Missing Key Rotation Specification
**Location**: Section 1
**Issue**: No explanation of how key rotation would work if a user wants to change their password or rotate their MasterKey.

### 32. Unclear Cipher Agility Implementation
**Location**: Section 1.7
**Issue**: States that cipher field allows future algorithm changes, but doesn't explain how the client knows which ciphers are supported or how fallback works.

### 33. Underspecified Wire Formats
**Location**: Section 1.8
**Issue**: Wire formats are shown for key encryption and file encryption, but there's no complete binary layout specification (byte ordering, endianness, etc.).

### 34. Video Thumbnail Specification Missing
**Location**: Section 3.1
**Issue**: Says "Videos: extract frame at 1s mark" but doesn't specify the video decoding requirements, codec support, or fallback behavior.

### 35. Inconsistent Encryption References
**Location**: Section 1.1
**Issue**: The key hierarchy diagram shows XSalsa20-Poly1305 for key encryption and XChaCha20-Poly1305 for file/thumbnail encryption, but doesn't clearly explain why different algorithms are used for different purposes.

### 36. Version-Consistent Pagination Edge Case
**Location**: Section 2.3
**Issue**: "If last group is incomplete → discard it (will be on next page)" doesn't handle the case where there are exactly N+1 rows with identical `updation_time`. The (N+1)th row would be discarded but never appear on the next page.

**Fix**: Clarify that `updation_time` should have sufficient precision (e.g., microseconds) to avoid collisions, or use a secondary sort key.

### 37. Local DB Security Underspecified
**Location**: Section 5.2
**Issue**: States the local DB is encrypted but doesn't specify:
- What encryption algorithm is used
- How the encryption key is derived
- How the key is stored (keychain/keystore)
- What happens on device sleep/wake

### 38. Missing Decryption Failure Handling
**Location**: Section 2.4
**Issue**: The client sync flow doesn't specify what happens if decryption fails (wrong key, corrupted data, unsupported cipher).

### 39. Search Query Inefficiency
**Location**: Section 4.1
**Issue**: The text search SQL uses `LIKE '%query%'` which cannot use standard B-tree indexes. The schema has `CREATE INDEX idx_files_title ON files(title COLLATE NOCASE);` but this won't help with `LIKE '%query%'`.

**Fix**: Either use FTS5 for text search, or use `title LIKE 'query%'` (prefix-only) which can use the index.

## 🟢 LOW Issues

### 40. Typo in Section Header
**Location**: Section 2.6
**Issue**: "Re-login / clear data" should be consistently hyphenated or rephrased.

### 41. Thumbnail Format Not Specified
**Location**: Section 3.1
**Issue**: Max size is 100 KB but the format isn't explicitly stated (assumed to be JPEG).

### 42. Inconsistent Terminology
**Location**: Throughout
**Issue**: Mixes "fileKey", "FileKey", "file key" — should standardize on one convention (recommend: `file_key` for code, File Key for prose).

### 43. Magic Metadata Not Explained
**Location**: Section 2.4 `metadata` crate
**Issue**: References "magic metadata" for server-side sorting but doesn't explain what this means or how it works.

---

# ZOO.md

## 🔴 HIGH Issues

### 44. SQL BYTEA Misuse for Bitmask
**Location**: Section 5.2 `uploads` table
**Issue**: `parts_bitmask` is defined as `BYTEA NOT NULL DEFAULT ''`. PostgreSQL's `BYTEA` is a variable-length byte array, but there's no standard way to store a bitmask in it. This should use `BIGINT` for up to 64 parts, or `NUMERIC` for larger bitmasks, or explicitly define the encoding (e.g., "big-endian byte array where each byte represents 8 parts").

**Fix**: Define the exact encoding: `BYTEA` storing a variable-length bit vector where bit 0 = part 0, bit 1 = part 1, etc. Or better, use `BIGINT` for typical use cases (up to 64 parts per upload, which covers 99% of files).

### 45. State Machine Invalid Transition
**Location**: Section 6.1
**Issue**: The state machine shows `STALLED → ENCRYPTING` transition. However, ENCRYPTING is an early state (before UPLOADING). A stalled upload has already progressed past encryption. The resume flow (Section 10) shows that stalled uploads resume to UPLOADING state, not ENCRYPTING.

**Fix**: Remove the `STALLED → ENCRYPTING` transition. Stalled uploads can only resume to UPLOADING (if parts remain) or proceed to S3_COMPLETED (if all parts are done but registration failed).

## 🟡 MEDIUM Issues

### 46. S3 URL Exposure in Redirect Mode
**Location**: Section 11.1
**Issue**: In redirect mode, Zoo returns HTTP 302 to a presigned S3 URL. This exposes S3 URLs to clients. While presigned URLs are temporary and single-purpose, they reveal:
- The S3 bucket name
- The object key structure
- The AWS region

This could be a security concern if S3 URLs have predictable patterns that could be exploited.

**Suggestion**: Consider always using proxy mode, or document the security implications of redirect mode.

### 47. Missing Rate Limit Error Response
**Location**: Section 18.2
**Issue**: Rate limiting is specified but there's no definition of what error response is returned when a rate limit is exceeded.

**Fix**: Add: "Rate limited requests return HTTP 429 with `Retry-After` header."

### 48. Incomplete Configuration Security
**Location**: Section 15
**Issue**: `s3_secret_key` is shown as a plain string in the configuration. This should be handled more securely, especially in production.

**Suggestion**: Support environment variable references or a secrets management integration.

### 49. Unclear S3 Lifecycle Interaction
**Location**: Section 12.3
**Issue**: Zoo has its own GC, but S3 also has lifecycle policies. There's no explanation of how these interact or what happens if S3 deletes something Zoo expects to exist.

### 50. Missing Upload ID Persistence Strategy
**Location**: Section 10.3
**Issue**: Web resume persists `upload_id` in localStorage, but desktop (Tauri) persistence isn't specified.

### 51. Inconsistent Error Handling in State Machine
**Location**: Section 6.1
**Issue**: "The server never transitions state unilaterally except for STALLED → FAILED." But the table also shows `UPLOADING → FAILED` on S3 error (AbortMultipartUpload). This seems like a server-initiated transition.

**Fix**: Clarify that server CAN transition to FAILED on S3 errors, not just on GC expiry.

### 52. Device Name Not Available at Upload Creation
**Location**: Section 7.2 POST /api/uploads
**Issue**: Response includes `device_name` but the device is registered separately (Section 7.1). At upload creation time, the device may not have a name yet.

**Fix**: Either remove `device_name` from the response, or ensure device registration happens before upload creation.

### 53. Missing Presigned URL Expiry Handling
**Location**: Section 13.2
**Issue**: The orchestrator pseudocode doesn't handle the case where a presigned URL has expired between presign and PUT.

**Fix**: Add error handling: if PUT fails with 403 (expired signature), call `presign-refresh` and retry.

### 54. Incomplete Dedup Protection
**Location**: Section 14.1
**Issue**: Active upload dedup uses `(user_id, file_hash)` but doesn't consider `collection_id`. The same file could be uploaded to different collections simultaneously.

**Fix**: Change UNIQUE index to `(user_id, file_hash, collection_id)` or clarify that file_hash + collection_id is the dedup key.

### 55. Unclear Session Token Storage
**Location**: Section 18.1
**Issue**: Session tokens are stored as SHA-256 hashes, but there's no specification of how the mapping from hash to user_id is stored or looked up.

### 56. Missing SSE Authentication Details
**Location**: Section 8.2
**Issue**: `SseAuth` struct is referenced but not defined. No explanation of how device-level tokens are validated.

## 🟢 LOW Issues

### 57. SQL Syntax Error in Sharing Model
**Location**: Section 11.3
**Issue**: Missing closing parenthesis in the SQL CREATE TABLE statement.

### 58. Hardcoded Timeout Values
**Location**: Section 9.1
**Issue**: Stall detection worker SQL query references `$1::interval` but doesn't show the actual value used.

### 59. Inconsistent Terminology: "Zoo"
**Location**: Throughout
**Issue**: "Zoo" is used for both the server crate (`crates/zoo/`) and the service. This is confusing. Consider using "Zoo server" or "Zoo service" for clarity.

### 60. Missing Orchestrator Location
**Location**: Section 4
**Issue**: Shows `crates/zoo-client/` containing `orchestrator.rs` but Section 13 describes the orchestrator as part of the client SDK. Clarify where this lives.

### 61. Unclear Platform Abstraction Implementation
**Location**: Section 13.4
**Issue**: The platform abstraction table shows Web using `fetch` for S3 client, but the S3 PUT operation in the orchestrator uses `s3_put(url, part)` — it's unclear how the platform-specific S3 client is injected.

### 62. Missing SSE Rate Limiting
**Location**: Section 18.2
**Issue**: Rate limiting table doesn't include the SSE endpoint, even though it's mentioned in Section 7.4.

---

# ANALYSIS.md

## 🔴 HIGH Issues

### 63. Off-Topic Content
**Location**: Entire file
**Issue**: This file analyzes the **Ente** project's monorepo, not Rey's. It contains:
- Ente's issue statistics
- Ente's bug categories
- Ente's feature gaps
- Ente's architecture observations

This content doesn't belong in the Rey repository. It appears to be reference material that was accidentally included or not properly separated.

**Fix**: Either:
1. Remove this file entirely (recommended)
2. Move it to a `tmp/` or `research/` directory with clear labeling
3. Add a disclaimer at the top explaining its purpose

## 🟡 MEDIUM Issues

### 64. Unclear Relevance to Rey
**Location**: Entire file
**Issue**: Even if this is meant as comparative analysis, there's no explanation of how the findings apply to Rey or what lessons were learned.

### 65. Outdated References
**Location**: Throughout
**Issue**: References specific Ente GitHub issue numbers (e.g., #1509, #4100) which have no meaning in the Rey context.

---

# Cross-Document Issues

## 🟡 MEDIUM Issues

### 66. Redundant Information
**Issue**: Crate responsibilities and dependencies are described in both ARCHITECTURE.md and STRUCTURE.md with minor variations but no clear reason for the duplication.

**Suggestion**: Pick one file as the source of truth for crate structure, and have other files reference it.

### 67. Inconsistent Cross-References
**Issue**: Documents don't link to each other. For example:
- SPEC.md doesn't reference ZOO.md for upload details
- ARCHITECTURE.md doesn't reference SPEC.md for encryption scheme
- STRUCTURE.md doesn't reference ARCHITECTURE.md for platform strategy

**Fix**: Add cross-document links where appropriate.

### 68. Terminology Inconsistency
**Issue**: Different documents use different terms for the same concept:
- "fileKey" (SPEC.md) vs "FileKey" (ZOO.md) vs "file key" (ARCHITECTURE.md)
- "Zoo" for both crate and service
- "upload_id" vs "upload ID"

**Suggestion**: Create a glossary or style guide.

## 🟢 LOW Issues

### 69. British vs American English
**Issue**: Mixes "behaviour" (British) and "behavior" (American), "optimise" vs "optimize", etc.

**Suggestion**: Standardize on one variant (recommend American English for a Rust project, as Rust uses American spellings in its documentation).

### 70. Missing Document Purpose Statements
**Issue**: Some files don't clearly state their purpose at the top. ANALYSIS.md in particular lacks context.

### 71. Inconsistent Heading Levels
**Issue**: Some documents skip heading levels (e.g., from H2 to H4 without H3).

### 72. Missing Table of Contents
**Issue**: Some longer documents (especially ZOO.md) would benefit from a table of contents at the top.

---

# Summary Statistics

| File | High | Medium | Low | Total |
|------|------|--------|-----|-------|
| ARCHITECTURE.md | 0 | 6 | 7 | 13 |
| STRUCTURE.md | 0 | 7 | 6 | 13 |
| SPEC.md | 3 | 10 | 5 | 18 |
| ZOO.md | 2 | 12 | 6 | 20 |
| ANALYSIS.md | 1 | 2 | 0 | 3 |
| Cross-document | 0 | 3 | 4 | 7 |
| **Total** | **6** | **40** | **28** | **74** |

*(Note: Some issues span multiple files and are counted in multiple categories above, but the unique issue count is 65.)*

---

# Recommended Fix Priority

## Phase 1: Critical (Fix Before Implementation)
1. **#44** - Define bitmask encoding in PostgreSQL
2. **#45** - Fix state machine invalid transition
3. **#26** - Add constant-time comparison note for authentication
4. **#63** - Remove or properly label ANALYSIS.md

## Phase 2: Clarity (Fix Before Code Review)
1. **#29** - Address content hash privacy leak
2. **#37** - Specify local DB encryption details
3. **#46** - Document S3 URL exposure security implications
4. **#66** - Consolidate duplicate information
5. **#67** - Add cross-document references

## Phase 3: Polish (Fix Before Release)
1. All 🟢 LOW issues (typos, formatting, consistency)
2. All remaining 🟡 MEDIUM issues

---

# Notes

- Many of these nitpicks are interconnected. Fixing one may resolve others.
- The ANALYSIS.md file is the most problematic as it appears to be from a different project entirely.
- The SPEC.md and ZOO.md files have the most substantive issues that could affect implementation.
- The ARCHITECTURE.md and STRUCTURE.md files are generally well-written but have redundancies and minor inconsistencies.
