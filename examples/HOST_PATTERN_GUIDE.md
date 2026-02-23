# Host Pattern Configuration Guide

This guide shows how to configure host patterns in both TOML and YAML formats.

## Basic Syntax

### TOML Format

```toml
[[plugins]]
pattern = "/api/*"
pattern_type = "glob"
response_source = { type = "file", path = "response.json" }
status_code = 200
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
```

### YAML Format

```yaml
plugins:
  - pattern: /api/*
    pattern_type: glob
    response_source:
      type: file
      path: response.json
    status_code: 200
    host_pattern:
      pattern: api.example.com
      pattern_type: exact
```

## Pattern Types

### 1. Exact Match

Matches the Host header exactly as specified.

**TOML:**
```toml
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
```

**YAML:**
```yaml
host_pattern:
  pattern: api.example.com
  pattern_type: exact
```

**Matches:**
- `api.example.com`

**Does NOT match:**
- `www.api.example.com`
- `api.example.com.evil.com`
- `API.EXAMPLE.COM` (case-sensitive)

### 2. Glob Pattern

Uses shell-style wildcards for flexible matching.

**TOML:**
```toml
host_pattern = { pattern = "*.example.com", pattern_type = "glob" }
```

**YAML:**
```yaml
host_pattern:
  pattern: "*.example.com"
  pattern_type: glob
```

**Wildcards:**
- `*` - Matches any sequence of characters
- `?` - Matches any single character
- `[abc]` - Matches any character in the set
- `[!abc]` - Matches any character NOT in the set

**Examples:**

| Pattern | Matches | Does NOT Match |
|---------|---------|----------------|
| `*.example.com` | `api.example.com`, `www.example.com` | `example.com`, `api.example.com.evil.com` |
| `api-*.example.com` | `api-v1.example.com`, `api-prod.example.com` | `api.example.com`, `apiv1.example.com` |
| `*.*.example.com` | `api.v1.example.com`, `www.prod.example.com` | `api.example.com` |
| `api?.example.com` | `api1.example.com`, `apiv.example.com` | `api.example.com`, `api12.example.com` |

### 3. Regex Pattern

Uses regular expressions for complex matching.

**TOML:**
```toml
host_pattern = { pattern = "^(api|www)\\.example\\.com$", pattern_type = "regex" }
```

**YAML:**
```yaml
host_pattern:
  pattern: "^(api|www)\\.example\\.com$"
  pattern_type: regex
```

**Important:** In YAML, use quotes around regex patterns to avoid parsing issues with special characters.

**Examples:**

| Pattern | Matches | Does NOT Match |
|---------|---------|----------------|
| `^api\\.example\\.com$` | `api.example.com` | `www.api.example.com`, `api.example.com.evil.com` |
| `^(api\|www)\\.example\\.com$` | `api.example.com`, `www.example.com` | `ftp.example.com` |
| `^api-v[0-9]+\\.example\\.com$` | `api-v1.example.com`, `api-v123.example.com` | `api-v.example.com`, `api-vX.example.com` |
| `.*\\.internal\\.com$` | `api.internal.com`, `db.prod.internal.com` | `internal.com`, `internal.com.evil.com` |

## Common Use Cases

### 1. Single Domain

**TOML:**
```toml
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
```

**YAML:**
```yaml
host_pattern:
  pattern: api.example.com
  pattern_type: exact
```

### 2. All Subdomains

**TOML:**
```toml
host_pattern = { pattern = "*.example.com", pattern_type = "glob" }
```

**YAML:**
```yaml
host_pattern:
  pattern: "*.example.com"
  pattern_type: glob
```

### 3. Multiple Specific Domains

**TOML:**
```toml
host_pattern = { pattern = "^(api|www|admin)\\.example\\.com$", pattern_type = "regex" }
```

**YAML:**
```yaml
host_pattern:
  pattern: "^(api|www|admin)\\.example\\.com$"
  pattern_type: regex
```

### 4. Environment-Specific Domains

**TOML:**
```toml
host_pattern = { pattern = "api-*.example.com", pattern_type = "glob" }
```

**YAML:**
```yaml
host_pattern:
  pattern: "api-*.example.com"
  pattern_type: glob
```

Matches: `api-dev.example.com`, `api-staging.example.com`, `api-prod.example.com`

### 5. Internal Domains Only

**TOML:**
```toml
host_pattern = { pattern = ".*\\.internal\\.com$", pattern_type = "regex" }
```

**YAML:**
```yaml
host_pattern:
  pattern: ".*\\.internal\\.com$"
  pattern_type: regex
```

## Combining with Other Filters

Host patterns can be combined with process-aware filters using AND logic.

**TOML:**
```toml
[[plugins]]
pattern = "/admin/*"
pattern_type = "glob"
response_source = { type = "file", path = "admin.json" }
status_code = 200
uid = 0  # Only root
host_pattern = { pattern = "admin.example.com", pattern_type = "exact" }
```

**YAML:**
```yaml
plugins:
  - pattern: /admin/*
    pattern_type: glob
    response_source:
      type: file
      path: admin.json
    status_code: 200
    uid: 0  # Only root
    host_pattern:
      pattern: admin.example.com
      pattern_type: exact
```

This plugin only applies when:
- Path matches `/admin/*` AND
- Request is from uid 0 (root) AND
- Host header is `admin.example.com`

## IP-Agnostic Mode

To use host patterns effectively, enable IP-agnostic mode by omitting the `ip` field:

**TOML:**
```toml
[interception]
port = 80
# ip field omitted - intercepts all IPs on port 80
```

**YAML:**
```yaml
interception:
  port: 80
  # ip field omitted - intercepts all IPs on port 80
```

This allows you to intercept traffic to multiple domains and filter by Host header.

## Testing Your Configuration

Test your host pattern configuration:

```bash
# Start the proxy
sudo ./mefirst --config examples/config-optional-ip.yaml

# Test with curl (sets Host header)
curl -H "Host: api.example.com" http://localhost:8080/api/test
curl -H "Host: www.example.com" http://localhost:8080/api/test
curl -H "Host: admin.example.com" http://localhost:8080/admin/test
```

## Troubleshooting

### Pattern Not Matching

1. Check that the Host header is being sent by the client
2. Verify pattern syntax (especially regex escaping)
3. Test pattern type (try `exact` first, then `glob`, then `regex`)
4. Check logs for pattern matching debug information

### Case Sensitivity

- All pattern types are case-sensitive by default
- For case-insensitive regex, use: `(?i)^api\\.example\\.com$`

### Special Characters in YAML

Always quote patterns with special characters in YAML:

```yaml
# Good
host_pattern:
  pattern: "*.example.com"
  pattern_type: glob

# Bad (may cause parsing errors)
host_pattern:
  pattern: *.example.com
  pattern_type: glob
```

## Complete Examples

See these example configurations:
- `examples/config-optional-ip.yaml` - IP-agnostic mode with host patterns
- `examples/config-optional-ip.toml` - Same in TOML format
- `examples/config-process-aware.yaml` - Process-aware routing with host patterns
- `examples/config-process-aware.toml` - Same in TOML format
