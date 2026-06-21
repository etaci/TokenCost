# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability, please follow these steps:

### 🔒 Private Disclosure

**DO NOT** open a public GitHub issue. Instead:

1. **Email us directly** at security@fusebox.dev
2. **Include the following information:**
   - Type of vulnerability
   - Full paths of affected source files
   - Location of the vulnerable code (tag/branch/commit or URL)
   - Step-by-step instructions to reproduce
   - Proof-of-concept or exploit code (if possible)
   - Impact assessment (what an attacker could do)

### 📧 What to Expect

- **Acknowledgment** - We'll acknowledge receipt within 48 hours
- **Updates** - We'll send updates every 5 business days
- **Timeline** - We aim to patch critical issues within 7 days
- **Credit** - We'll credit you in the security advisory (unless you prefer to remain anonymous)

### 🛡️ Security Measures

Fusebox implements several security measures:

#### 1. API Key Protection
- API keys are never logged
- Keys are not included in error messages
- Keys are not stored in the database

#### 2. Input Validation
- All inputs are validated and sanitized
- SQL injection protection via parameterized queries
- Path traversal prevention

#### 3. Rate Limiting
- Circuit breaker prevents runaway requests
- Budget limits enforce cost caps
- Tenant isolation prevents cross-tenant access

#### 4. Dependency Security
- Regular `cargo audit` runs
- Minimal dependency footprint
- Only well-maintained dependencies

### 🔍 Known Security Considerations

#### Database Access
- SQLite: File-based, ensure proper file permissions
- Postgres: Use strong passwords and TLS connections

#### API Keys
- Store in environment variables, not in config files
- Use secrets management in production (Vault, AWS Secrets Manager)

#### Network Security
- Deploy behind a reverse proxy with TLS
- Use firewall rules to restrict access
- Consider VPC/private networks

#### Multi-Tenancy
- Tenant IDs are used for isolation
- Always validate tenant access in custom integrations

### 📚 Security Best Practices

When deploying Fusebox:

```yaml
# ✅ Good - Environment variables
environment:
  OPENAI_API_KEY: ${OPENAI_API_KEY}

# ❌ Bad - Hardcoded in config
providers:
  openai:
    api_key: sk-hardcoded-key  # DON'T DO THIS
```

```bash
# ✅ Good - Run as non-root
docker run --user 1000:1000 fusebox:latest

# ❌ Bad - Run as root
docker run fusebox:latest
```

```yaml
# ✅ Good - TLS enabled
ingress:
  tls:
    - secretName: fusebox-tls
      hosts:
        - fusebox.example.com

# ❌ Bad - No TLS
# (allows man-in-the-middle attacks)
```

### 🚨 Vulnerability Disclosure Policy

We follow responsible disclosure:

1. **Report received** - Private acknowledgment sent
2. **Validation** - We verify and assess impact
3. **Patch development** - We create and test a fix
4. **Coordinated release** - We notify you before public disclosure
5. **Public disclosure** - Security advisory published with credit

### 📞 Contact

- **Security Email**: security@fusebox.dev
- **GPG Key**: (coming soon)
- **Response Time**: 48 hours for acknowledgment

### 🏆 Hall of Fame

We'll recognize security researchers who responsibly disclose vulnerabilities:

*(No vulnerabilities reported yet - be the first!)*

---

Thank you for helping keep Fusebox secure! 🛡️
